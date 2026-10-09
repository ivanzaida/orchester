#![allow(unsafe_code)]
//! The entry changes on Windows. The standard library deletes and renames by path, which follows a link or junction
//! swapped in after the check, and has no call that deletes or renames through a handle; so this module calls the
//! Win32 functions itself (FFI without a safe wrapper, AGENTS.md rule 9). The entry is opened with
//! `FILE_FLAG_OPEN_REPARSE_POINT`, so a link at its end is opened itself; the path the handle reached is read back
//! (`GetFinalPathNameByHandleW`) and must be the one checked, so no junction on the way led elsewhere; then the entry
//! is deleted (`FileDispositionInfo`) or renamed through that handle, never replacing an entry. The target's folder
//! is opened the same way and must be a plain folder, no junction or other reparse point; it is held open without
//! sharing deletion, and the rename names only the new name relative to that handle (`RootDirectory`), so no path is
//! resolved again after the check. `SetFileInformationByHandle` refuses a name relative to a folder handle
//! (`ERROR_INVALID_PARAMETER`), so the rename calls `NtSetInformationFile` (`FileRenameInformation`), which takes it.

use std::{
  ffi::OsString,
  io, mem,
  os::windows::ffi::{OsStrExt, OsStringExt},
  path::{Path, PathBuf},
  ptr,
};

use windows_sys::{
  Wdk::Storage::FileSystem::{FILE_RENAME_INFORMATION, FileRenameInformation, NtSetInformationFile},
  Win32::{
    Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE, RtlNtStatusToDosError},
    Storage::FileSystem::{
      BY_HANDLE_FILE_INFORMATION, CreateFileW, DELETE, FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_REPARSE_POINT,
      FILE_DISPOSITION_INFO, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT, FILE_LIST_DIRECTORY,
      FILE_NAME_NORMALIZED, FILE_READ_ATTRIBUTES, FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE,
      FileDispositionInfo, GetFileInformationByHandle, GetFinalPathNameByHandleW, OPEN_EXISTING,
      SetFileInformationByHandle, VOLUME_NAME_DOS,
    },
    System::IO::IO_STATUS_BLOCK,
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
    let capacity = u32::try_from(buffer.len()).map_err(|_| io::Error::other("a path longer than Windows reports"))?;
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
  let (folder, link) = kind(&handle)?;
  if folder && !link {
    return Err(EntryError::Folder);
  }
  Ok(handle)
}

/// Whether the entry `handle` opened is a folder, and whether it is a reparse point (a link or junction).
fn kind(handle: &Handle) -> io::Result<(bool, bool)> {
  // SAFETY: an all-zero `BY_HANDLE_FILE_INFORMATION` is a valid value of the plain C struct.
  let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { mem::zeroed() };
  // SAFETY: `handle.0` is open and `info` is a writable struct of the type the call fills.
  if unsafe { GetFileInformationByHandle(handle.0, &raw mut info) } == 0 {
    return Err(io::Error::last_os_error());
  }
  Ok((
    info.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY != 0,
    info.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT != 0,
  ))
}

/// The folder `folder`, inside `root`, opened to receive a renamed entry: a plain folder, held without sharing
/// deletion so it cannot be renamed or replaced until the rename is done.
fn open_target_folder(root: &Path, folder: &Path) -> Result<Handle, EntryError> {
  let held = open(
    folder,
    FILE_LIST_DIRECTORY | FILE_READ_ATTRIBUTES,
    FILE_SHARE_READ | FILE_SHARE_WRITE,
  )?;
  if !same(&reached(&held)?, folder, root) {
    return Err(EntryError::Moved);
  }
  // A junction swapped in is opened itself: it reached its own path, but a rename into it would land where it leads.
  if kind(&held)? != (true, false) {
    return Err(EntryError::Moved);
  }
  Ok(held)
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
  let held = open_target_folder(root, to.parent().ok_or(EntryError::Moved)?)?;
  let name: Vec<u16> = to.file_name().ok_or(EntryError::Moved)?.encode_wide().collect();
  let name_bytes = u32::try_from(name.len() * mem::size_of::<u16>()).map_err(|_| EntryError::Moved)?;
  let header = mem::offset_of!(FILE_RENAME_INFORMATION, FileName);
  let size = (header + name.len() * mem::size_of::<u16>()).max(mem::size_of::<FILE_RENAME_INFORMATION>());
  // A buffer of `u64`s keeps the struct's alignment (it holds a handle).
  let mut buffer = vec![0_u64; size.div_ceil(mem::size_of::<u64>())];
  let info = buffer.as_mut_ptr().cast::<FILE_RENAME_INFORMATION>();
  // SAFETY: `buffer` is zeroed, aligned for `FILE_RENAME_INFORMATION` and at least `size` bytes long, so the header
  // fields and `name.len()` wide characters from `FileName` on lie inside it; a zero `ReplaceIfExists` replaces
  // nothing, and `RootDirectory` is the open folder `held` that the bare name is relative to.
  unsafe {
    (&raw mut (*info).RootDirectory).write(held.0);
    (&raw mut (*info).FileNameLength).write(name_bytes);
    ptr::copy_nonoverlapping(name.as_ptr(), (&raw mut (*info).FileName).cast::<u16>(), name.len());
  }
  let size = u32::try_from(size).map_err(|_| EntryError::Moved)?;
  // SAFETY: an all-zero `IO_STATUS_BLOCK` is a valid value of the plain C struct.
  let mut status_block: IO_STATUS_BLOCK = unsafe { mem::zeroed() };
  // SAFETY: `handle.0` is open with `DELETE` access, `held.0` stays open until after the call, `info` points at a
  // `FILE_RENAME_INFORMATION` of `size` bytes that `buffer` keeps alive through it, and `status_block` is writable.
  let status = unsafe {
    NtSetInformationFile(
      handle.0,
      &raw mut status_block,
      info.cast(),
      size,
      FileRenameInformation,
    )
  };
  drop(held);
  if status < 0 {
    // SAFETY: the call only maps a status code to a Win32 error code.
    let code = unsafe { RtlNtStatusToDosError(status) };
    return Err(io::Error::from_raw_os_error(i32::try_from(code).unwrap_or(i32::MAX)).into());
  }
  Ok(())
}
