#![allow(unsafe_code)]
//! Windows has no safe std wrapper for handle-bound deletion or link-count validation.
//! All ancestors are held without sharing deletion; files deny writes/deletion while compared.

use std::{
  ffi::OsStr,
  fs::{File, OpenOptions},
  io, mem,
  os::windows::{
    fs::{MetadataExt, OpenOptionsExt},
    io::AsRawHandle,
  },
  path::{Component, Path, PathBuf},
  sync::Arc,
};

use windows_sys::Win32::{
  Foundation::{GENERIC_READ, GENERIC_WRITE},
  Storage::FileSystem::{
    BY_HANDLE_FILE_INFORMATION, DELETE, FILE_DISPOSITION_INFO, FILE_FLAG_BACKUP_SEMANTICS,
    FILE_FLAG_OPEN_REPARSE_POINT, FILE_SHARE_READ, FILE_SHARE_WRITE, FileDispositionInfo, GetFileInformationByHandle,
    GetFinalPathNameByHandleW, SetFileInformationByHandle,
  },
};

use super::super::{LocalError, LocalGitError};

pub struct Directory {
  file: File,
  path: PathBuf,
  _parent: Option<Arc<Self>>,
}

impl Directory {
  pub fn open(root: &Path) -> Result<Arc<Self>, LocalGitError> {
    orchester_core::permission::stays_local(root).map_err(|_| LocalError::Unsupported)?;
    if !root.is_absolute() {
      return Err(LocalError::Unsupported.into());
    }
    let mut anchor = PathBuf::new();
    let mut directory = None;
    for part in root.components() {
      match part {
        Component::Prefix(prefix) => anchor.push(prefix.as_os_str()),
        Component::RootDir => {
          anchor.push(part.as_os_str());
          directory = Some(Self::at(&anchor, None)?);
        }
        Component::Normal(name) => {
          let previous = directory.ok_or(LocalError::Unsupported)?;
          directory = previous.child(name, false)?;
          if directory.is_none() {
            return Err(LocalGitError::Io);
          }
        }
        Component::CurDir => {}
        Component::ParentDir => {
          let current = directory.ok_or(LocalError::Unsupported)?;
          directory = current._parent.clone();
          if directory.is_none() {
            return Err(LocalError::Unsupported.into());
          }
        }
      }
    }
    directory.ok_or(LocalError::Unsupported.into())
  }

  fn at(path: &Path, parent: Option<Arc<Self>>) -> Result<Arc<Self>, LocalGitError> {
    let file = OpenOptions::new()
      .access_mode(GENERIC_READ)
      .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
      .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS)
      .open(path)
      .map_err(map)?;
    let metadata = file.metadata().map_err(map)?;
    if !metadata.is_dir() || metadata.file_attributes() & 0x400 != 0 {
      return Err(LocalError::Unsupported.into());
    }
    ensure_location(&file, path)?;
    Ok(Arc::new(Self {
      file,
      path: path.to_owned(),
      _parent: parent,
    }))
  }

  pub fn path(&self) -> &Path {
    &self.path
  }

  pub fn child(self: &Arc<Self>, name: &OsStr, create: bool) -> Result<Option<Arc<Self>>, LocalGitError> {
    if Path::new(name).components().count() != 1
      || !matches!(Path::new(name).components().next(), Some(Component::Normal(_)))
    {
      return Err(LocalError::Unsupported.into());
    }
    let _freeze = self.freeze()?;
    let path = self.path.join(name);
    match Self::at(&path, Some(self.clone())) {
      Ok(directory) => Ok(Some(directory)),
      Err(LocalGitError::Io) if create => {
        match std::fs::create_dir(&path) {
          Ok(()) => {}
          Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
          Err(error) => return Err(map(error)),
        }
        Self::at(&path, Some(self.clone())).map(Some)
      }
      Err(LocalGitError::Io) if !path.exists() => Ok(None),
      Err(error) => Err(error),
    }
  }

  pub(super) fn freeze(&self) -> Result<File, LocalGitError> {
    self.ensure_current()?;
    let file = OpenOptions::new()
      .access_mode(GENERIC_READ)
      .share_mode(FILE_SHARE_READ)
      .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS)
      .open(&self.path)
      .map_err(map)?;
    ensure_location(&file, &self.path)?;
    let metadata = file.metadata().map_err(map)?;
    if !metadata.is_dir() || metadata.file_attributes() & 0x400 != 0 {
      return Err(LocalError::Unsupported.into());
    }
    if object_identity(&file)? != object_identity(&self.file)? {
      return Err(LocalError::Stale.into());
    }
    Ok(file)
  }

  pub fn ensure_current(&self) -> Result<(), LocalGitError> {
    if let Some(parent) = &self._parent {
      parent.ensure_current()?;
    }
    ensure_location(&self.file, &self.path)?;
    let metadata = self.file.metadata().map_err(map)?;
    if !metadata.is_dir() || metadata.file_attributes() & 0x400 != 0 {
      return Err(LocalError::Unsupported.into());
    }
    Ok(())
  }

  pub(super) fn same_entry(&self, _name: &OsStr, _file: &File) -> Result<(), LocalGitError> {
    // File and every ancestor deny sharing deletion; their entries cannot be substituted.
    self.ensure_current()
  }

  pub(super) fn open_regular(&self, name: &OsStr, writing: bool, create: bool) -> Result<Option<File>, LocalGitError> {
    let _freeze = self.freeze()?;
    let rights = GENERIC_READ | if writing { GENERIC_WRITE | DELETE } else { 0 };
    let mut options = OpenOptions::new();
    options
      .read(true)
      .write(writing)
      .access_mode(rights)
      .share_mode(FILE_SHARE_READ)
      .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS);
    if create {
      options.create_new(true);
    }
    match options.open(self.path.join(name)) {
      Ok(file) => {
        ensure_location(&file, &self.path.join(name))?;
        Ok(Some(file))
      }
      Err(error) if error.kind() == io::ErrorKind::NotFound && !create => Ok(None),
      Err(error) if error.kind() == io::ErrorKind::AlreadyExists => Err(LocalError::Collision.into()),
      Err(error) => Err(map(error)),
    }
  }
}

fn map(_error: io::Error) -> LocalGitError {
  LocalGitError::Io
}

pub(super) fn validate_regular(file: &File) -> Result<(), LocalGitError> {
  // SAFETY: the C record accepts zero initialization and the live handle fills the entire record.
  let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { mem::zeroed() };
  // SAFETY: file owns a valid live handle and info is a writable correctly-sized C record.
  if unsafe { GetFileInformationByHandle(file.as_raw_handle(), &raw mut info) } == 0 {
    return Err(LocalGitError::Io);
  }
  if info.dwFileAttributes & (0x400 | 0x10) != 0 || info.nNumberOfLinks != 1 {
    return Err(LocalError::Unsupported.into());
  }
  Ok(())
}

pub(super) fn executable(_file: &File) -> Result<bool, LocalGitError> {
  Ok(false)
}
pub(super) fn set_executable(_file: &File, _value: bool) -> Result<(), LocalGitError> {
  // Windows has no POSIX working-file executable mode; Git index mode is set separately.
  Ok(())
}
pub(super) fn remove(file: &File) -> Result<(), LocalGitError> {
  let disposition = FILE_DISPOSITION_INFO { DeleteFile: true };
  let size = mem::size_of::<FILE_DISPOSITION_INFO>() as u32;
  // SAFETY: the owned regular handle was opened with DELETE and without sharing deletion/writes.
  if unsafe {
    SetFileInformationByHandle(
      file.as_raw_handle(),
      FileDispositionInfo,
      (&raw const disposition).cast(),
      size,
    )
  } == 0
  {
    return Err(LocalGitError::Io);
  }
  Ok(())
}

pub(super) fn pin_store_entry(directory: &Arc<Directory>, name: &OsStr, create: bool) -> Result<File, LocalGitError> {
  let _freeze = directory.freeze()?;
  let path = directory.path.join(name);
  let open = || {
    OpenOptions::new()
      .access_mode(GENERIC_READ)
      .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
      .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS)
      .open(&path)
  };
  let file = match open() {
    Ok(file) => file,
    Err(error) if error.kind() == io::ErrorKind::NotFound && create => {
      match OpenOptions::new()
        .write(true)
        .create_new(true)
        .access_mode(GENERIC_READ)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
        .open(&path)
      {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => open().map_err(map)?,
        Err(error) => return Err(map(error)),
      }
    }
    Err(error) => return Err(map(error)),
  };
  validate_regular(&file)?;
  ensure_location(&file, &path)?;
  directory.ensure_current()?;
  Ok(file)
}
// Verify the opened object came from our pinned chain, including any transient ancestor reparse.
fn ensure_location(file: &File, expected: &Path) -> Result<(), LocalGitError> {
  let mut buffer = vec![0u16; 32768];
  // SAFETY: file owns a live handle; the buffer holds the stated UTF-16 capacity, flags select normalized DOS names.
  let length = unsafe { GetFinalPathNameByHandleW(file.as_raw_handle(), buffer.as_mut_ptr(), 32768, 0) };
  if length == 0 || length >= 32768 {
    return Err(LocalError::Unsupported.into());
  }
  let actual = String::from_utf16(&buffer[..length as usize]).map_err(|_| LocalError::Unsupported)?;
  let actual = actual.strip_prefix("\\\\?\\").unwrap_or(&actual);
  let expected = expected.to_str().ok_or(LocalError::Unsupported)?.replace('/', "\\");
  let expected = expected.strip_prefix(r"\\?\").unwrap_or(&expected);
  if !actual
    .trim_end_matches('\\')
    .eq_ignore_ascii_case(expected.trim_end_matches('\\'))
  {
    return Err(LocalError::Stale.into());
  }
  Ok(())
}
fn object_identity(file: &File) -> Result<(u32, u32, u32), LocalGitError> {
  // SAFETY: this C record accepts zero initialization; the live handle fills it on success.
  let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { mem::zeroed() };
  // SAFETY: file owns a live handle and info is a correctly-sized writable C record.
  if unsafe { GetFileInformationByHandle(file.as_raw_handle(), &raw mut info) } == 0 {
    return Err(LocalGitError::Io);
  }
  Ok((info.dwVolumeSerialNumber, info.nFileIndexHigh, info.nFileIndexLow))
}
