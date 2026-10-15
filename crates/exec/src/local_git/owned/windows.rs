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
    FILE_FLAG_OPEN_REPARSE_POINT, FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE, FileDispositionInfo,
    GetFileInformationByHandle, GetFinalPathNameByHandleW, SetFileInformationByHandle,
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

  /// Only execution metadata calls this after retaining a nonempty anchor and the complete parent chain.
  /// No-delete sharing prevents rename; the held child prevents empty-directory reparse conversion.
  pub(super) fn ensure_pinned(&self) -> Result<(), LocalGitError> {
    let metadata = self.file.metadata().map_err(map)?;
    if !metadata.is_dir() || metadata.file_attributes() & 0x400 != 0 {
      return Err(LocalError::Unsupported.into());
    }
    Ok(())
  }

  pub(crate) fn aliases(&self, left: &OsStr, right: &OsStr) -> Result<bool, LocalGitError> {
    let Some(left) = self.open_regular(left, false, false)? else {
      return Ok(false);
    };
    let Some(right) = self.open_regular(right, false, false)? else {
      return Ok(false);
    };
    for file in [&left, &right] {
      if file.metadata().map_err(map)?.file_attributes() & 0x400 != 0 {
        return Err(LocalError::Unsupported.into());
      }
    }
    Ok(object_identity(&left)? == object_identity(&right)?)
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
  let info = information(file)?;
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

/// How an entry is pinned.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Pin {
  /// An existing file, held against deletion and rename.
  Existing,
  /// SQLite's `-wal`, `-shm`, `-journal`: created when missing so it is a validated regular file before SQLite opens
  /// it, and shared for deletion: SQLite in another process (the runner, another app) deletes its WAL when its last
  /// connection closes and must not stall on our pin.
  Sidecar,
  /// An empty file created and held against deletion, which keeps its directory nonempty: NTFS refuses to turn a
  /// nonempty directory into a junction.
  Anchor,
}

/// How long a sidecar SQLite deletes while it is being pinned is pinned again before pinning fails (as long as
/// SQLite itself waits for a busy database), and the pause before trying again while its deletion is pending.
const SIDECAR_WAIT: std::time::Duration = std::time::Duration::from_secs(5);
const SIDECAR_PAUSE: std::time::Duration = std::time::Duration::from_millis(5);

/// Pins the entry `name` of `directory` as `pin` says.
pub(super) fn pin_store_entry(directory: &Arc<Directory>, name: &OsStr, pin: Pin) -> Result<File, LocalGitError> {
  let _freeze = directory.freeze()?;
  let path = directory.path.join(name);
  let deadline = std::time::Instant::now() + SIDECAR_WAIT;
  while std::time::Instant::now() < deadline {
    let file = match open_store_entry(&path, pin) {
      // SQLite is deleting the sidecar while one of its handles is still open: the name is free once it closes.
      Err(error) if pin == Pin::Sidecar && error.kind() == io::ErrorKind::PermissionDenied => {
        std::thread::sleep(SIDECAR_PAUSE);
        continue;
      }
      opened => opened.map_err(map)?,
    };
    match validate_regular(&file).and_then(|()| ensure_location(&file, &path)) {
      // SQLite's last connection deleted the sidecar since it was opened here: its name is free again and the
      // next connection creates it anew. Pin that one.
      Err(_) if pin == Pin::Sidecar && information(&file)?.nNumberOfLinks == 0 => continue,
      checked => checked?,
    }
    directory.ensure_current()?;
    return Ok(file);
  }
  Err(LocalGitError::Io)
}

/// Opens the entry at `path`, creating it first if `pin` allows that.
fn open_store_entry(path: &Path, pin: Pin) -> io::Result<File> {
  let share = if pin == Pin::Sidecar {
    FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE
  } else {
    FILE_SHARE_READ | FILE_SHARE_WRITE
  };
  let open = || {
    OpenOptions::new()
      .access_mode(GENERIC_READ)
      .share_mode(share)
      .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS)
      .open(path)
  };
  match open() {
    Err(error) if error.kind() == io::ErrorKind::NotFound && pin != Pin::Existing => {
      match OpenOptions::new()
        .write(true)
        .create_new(true)
        .access_mode(GENERIC_READ)
        .share_mode(share)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)
      {
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => open(),
        created => created,
      }
    }
    opened => opened,
  }
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
  let info = information(file)?;
  Ok((info.dwVolumeSerialNumber, info.nFileIndexHigh, info.nFileIndexLow))
}
/// What Windows records of the object `file` holds: its attributes, links and identity.
fn information(file: &File) -> Result<BY_HANDLE_FILE_INFORMATION, LocalGitError> {
  // SAFETY: this C record accepts zero initialization; the live handle fills it on success.
  let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { mem::zeroed() };
  // SAFETY: file owns a live handle and info is a correctly-sized writable C record.
  if unsafe { GetFileInformationByHandle(file.as_raw_handle(), &raw mut info) } == 0 {
    return Err(LocalGitError::Io);
  }
  Ok(info)
}
