#![allow(unsafe_code)]
//! The entry changes on Windows. The standard library deletes and renames by path, which follows a link or junction
//! swapped in after the check, and has no call that deletes or renames through a handle; so this module calls the
//! Win32 functions itself (FFI without a safe wrapper, AGENTS.md rule 9). The entry is opened with
//! `FILE_FLAG_OPEN_REPARSE_POINT`, so a link at its end is opened itself; the path the handle reached is read back
//! (`GetFinalPathNameByHandleW`) and must be the one checked, so no junction on the way led elsewhere; then the entry
//! is deleted (`FileDispositionInfo`) or renamed (`FileRenameInfo`, never replacing an entry) through that handle. The
//! target's folder is held open without sharing deletion while the rename runs, after its own path was checked the
//! same way.

use std::{
  ffi::OsString,
  io, mem,
  os::windows::ffi::{OsStrExt, OsStringExt},
  path::{Path, PathBuf},
  ptr,
};

use windows_sys::Win32::{
  Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE},
  Storage::FileSystem::{
    BY_HANDLE_FILE_INFORMATION, CreateFileW, DELETE, FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_REPARSE_POINT,
    FILE_DISPOSITION_INFO, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT, FILE_LIST_DIRECTORY,
    FILE_NAME_NORMALIZED, FILE_READ_ATTRIBUTES, FILE_RENAME_INFO, FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE,
    FileDispositionInfo, FileRenameInfo, GetFileInformationByHandle, GetFinalPathNameByHandleW, OPEN_EXISTING,
    SetFileInformationByHandle, VOLUME_NAME_DOS,
  },
};

use super::EntryError;

/// An open handle, closed when dropped.
struct Handle(HANDLE);

impl Drop for Handle {
  fn drop(&mut self) {
    // SAFETY: `self.0` is a valid handle `open` returned and nothing else closes it.
    unsafe {
      CloseHandle(self.0);
    }
  }
}

/// `path` as a NUL-terminated wide string, with the `\\?\` prefix so that long paths work.
fn wide(path: &Path) -> Vec<u16> {
  let text = path.as_os_str();
  let mut wide: Vec<u16> = if text.to_string_lossy().starts_with(r"\\") {
    text.encode_wide().collect()
  } else {
    r"\\?\".encode_utf16().chain(text.encode_wide()).collect()
  };
  wide.push(0);
  wide
}

/// The entry at `path`, opened without following a link at its end, for `access`, sharing `share`.
fn open(path: &Path, access: u32, share: u32) -> io::Result<Handle> {
  let name = wide(path);
  // SAFETY: `name` is a NUL-terminated wide string that outlives the call; the security attributes and the template
  // are null, as the function allows.
  let handle = unsafe {
    CreateFileW(
      name.as_ptr(),
      access,
      share,
      ptr::null(),
      OPEN_EXISTING,
      FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS,
      ptr::null_mut(),
    )
  };
  if handle == INVALID_HANDLE_VALUE {
    return Err(io::Error::last_os_error());
  }
  Ok(Handle(handle))
}

/// The path `handle` reached, without the `\\?\` prefix.
fn reached(handle: &Handle) -> io::Result<PathBuf> {
  let mut buffer = vec![0_u16; 1024];
  loop {
    let capacity = u32::try_from(buffer.len()).unwrap_or(u32::MAX);
    // SAFETY: `handle.0` is open and `buffer` holds `capacity` wide characters for the call to write.
    let length = unsafe {
      GetFinalPathNameByHandleW(
        handle.0,
        buffer.as_mut_ptr(),
        capacity,
        FILE_NAME_NORMALIZED | VOLUME_NAME_DOS,
      )
    };
    let length = usize::try_from(length).unwrap_or(usize::MAX);
    if length == 0 {
      return Err(io::Error::last_os_error());
    }
    if length < buffer.len() {
      buffer.truncate(length);
      let text = OsString::from_wide(&buffer).to_string_lossy().into_owned();
      let plain = match text.strip_prefix(r"\\?\UNC\") {
        Some(share) => format!(r"\\{share}"),
        None => text.strip_prefix(r"\\?\").unwrap_or(&text).to_owned(),
      };
      return Ok(PathBuf::from(plain));
    }
    buffer.resize(length + 1, 0);
  }
}

/// Whether `reached` is `expected`, as Windows compares names, and lies in `root`.
fn same(reached: &Path, expected: &Path, root: &Path) -> bool {
  let text = |path: &Path| path.to_string_lossy().to_lowercase();
  text(reached) == text(expected) && text(reached).starts_with(&text(root))
}

/// The entry at `path`, inside `root`, opened for deletion; refused when it is a folder (a link is no folder here).
fn open_entry(root: &Path, path: &Path) -> Result<Handle, EntryError> {
  let handle = open(
    path,
    DELETE | FILE_READ_ATTRIBUTES,
    FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
  )?;
  if !same(&reached(&handle)?, path, root) {
    return Err(EntryError::Moved);
  }
  // SAFETY: an all-zero `BY_HANDLE_FILE_INFORMATION` is a valid value of the plain C struct.
  let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { mem::zeroed() };
  // SAFETY: `handle.0` is open and `info` is a writable struct of the type the call fills.
  if unsafe { GetFileInformationByHandle(handle.0, &raw mut info) } == 0 {
    return Err(io::Error::last_os_error().into());
  }
  let folder = info.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY != 0;
  let link = info.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT != 0;
  if folder && !link {
    return Err(EntryError::Folder);
  }
  Ok(handle)
}

pub(super) fn remove(root: &Path, path: &Path) -> Result<(), EntryError> {
  let handle = open_entry(root, path)?;
  let disposition = FILE_DISPOSITION_INFO { DeleteFile: true };
  let size = u32::try_from(mem::size_of::<FILE_DISPOSITION_INFO>()).expect("a small struct");
  // SAFETY: `handle.0` is open with `DELETE` access and `disposition` is the struct `FileDispositionInfo` takes,
  // `size` bytes long.
  let done =
    unsafe { SetFileInformationByHandle(handle.0, FileDispositionInfo, (&raw const disposition).cast(), size) };
  if done == 0 {
    return Err(io::Error::last_os_error().into());
  }
  Ok(())
}

pub(super) fn rename(root: &Path, from: &Path, to: &Path) -> Result<(), EntryError> {
  let handle = open_entry(root, from)?;
  let folder = to.parent().ok_or(EntryError::Moved)?;
  // Held without sharing deletion, the target's folder cannot be renamed or replaced until the rename is done.
  let held = open(folder, FILE_LIST_DIRECTORY, FILE_SHARE_READ | FILE_SHARE_WRITE)?;
  if !same(&reached(&held)?, folder, root) {
    return Err(EntryError::Moved);
  }
  let name: Vec<u16> = wide(to);
  // The name's length in bytes, without its NUL.
  let name_bytes = (name.len() - 1) * mem::size_of::<u16>();
  let header = mem::offset_of!(FILE_RENAME_INFO, FileName);
  let size = header + name.len() * mem::size_of::<u16>();
  // A buffer of `u64`s keeps the struct's alignment (it holds a handle).
  let mut buffer = vec![0_u64; size.div_ceil(mem::size_of::<u64>())];
  let info = buffer.as_mut_ptr().cast::<FILE_RENAME_INFO>();
  // SAFETY: `buffer` is zeroed, aligned for `FILE_RENAME_INFO` and at least `size` bytes long, so the header fields
  // and `name.len()` wide characters from `FileName` on lie inside it; a zero `Flags` replaces nothing and a null
  // `RootDirectory` with a full path is what the call takes.
  unsafe {
    (&raw mut (*info).FileNameLength).write(u32::try_from(name_bytes).map_err(|_| EntryError::Moved)?);
    ptr::copy_nonoverlapping(name.as_ptr(), (&raw mut (*info).FileName).cast::<u16>(), name.len());
  }
  let size = u32::try_from(size).map_err(|_| EntryError::Moved)?;
  // SAFETY: `handle.0` is open with `DELETE` access and `info` points at a `FILE_RENAME_INFO` of `size` bytes that
  // `buffer` keeps alive through the call.
  let done = unsafe { SetFileInformationByHandle(handle.0, FileRenameInfo, info.cast(), size) };
  drop(held);
  if done == 0 {
    return Err(io::Error::last_os_error().into());
  }
  Ok(())
}
