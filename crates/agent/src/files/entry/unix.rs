//! The entry changes on Unix: every folder from the root down is opened relative to the one before with `O_NOFOLLOW`,
//! so no link on the way is followed, and the entry is deleted or renamed relative to its open folder (`unlinkat`,
//! `renameat` with `RENAME_NOREPLACE` on Linux, `RENAME_EXCL` on macOS), which acts on the link itself when it is one.

use std::{
  ffi::OsString,
  io,
  os::fd::OwnedFd,
  path::{Component, Path},
};

use rustix::fs::{AtFlags, CWD, FileType, Mode, OFlags, RenameFlags, openat, renameat_with, statat, unlinkat};

use super::EntryError;

/// The folder `folder`, inside `root`, opened down from `root` without following a link.
fn open_folder(root: &Path, folder: &Path) -> Result<OwnedFd, EntryError> {
  let flags = OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::RDONLY;
  let inside = folder.strip_prefix(root).map_err(|_| EntryError::Moved)?;
  let mut open = openat(CWD, root, flags, Mode::empty()).map_err(io::Error::from)?;
  for component in inside.components() {
    let Component::Normal(name) = component else {
      return Err(EntryError::Moved);
    };
    open = openat(&open, name, flags, Mode::empty()).map_err(|error| match error {
      rustix::io::Errno::LOOP | rustix::io::Errno::NOTDIR => EntryError::Moved,
      other => EntryError::Io(other.into()),
    })?;
  }
  Ok(open)
}

/// The open folder of `path` and its last name, refused when the entry is a folder.
fn entry(root: &Path, path: &Path) -> Result<(OwnedFd, OsString), EntryError> {
  let (Some(folder), Some(name)) = (path.parent(), path.file_name()) else {
    return Err(EntryError::Moved);
  };
  let folder = open_folder(root, folder)?;
  let stat = statat(&folder, name, AtFlags::SYMLINK_NOFOLLOW).map_err(io::Error::from)?;
  if FileType::from_raw_mode(stat.st_mode) == FileType::Directory {
    return Err(EntryError::Folder);
  }
  Ok((folder, name.to_owned()))
}

pub(super) fn remove(root: &Path, path: &Path) -> Result<(), EntryError> {
  let (folder, name) = entry(root, path)?;
  // Without `REMOVEDIR` a folder swapped in since the check is refused by the system itself.
  unlinkat(&folder, name.as_os_str(), AtFlags::empty()).map_err(|error| EntryError::from(io::Error::from(error)))
}

pub(super) fn rename(root: &Path, from: &Path, to: &Path) -> Result<(), EntryError> {
  let (source, name) = entry(root, from)?;
  let (Some(target), Some(target_name)) = (to.parent(), to.file_name()) else {
    return Err(EntryError::Moved);
  };
  let target = open_folder(root, target)?;
  renameat_with(&source, name.as_os_str(), &target, target_name, RenameFlags::NOREPLACE)
    .map_err(|error| EntryError::from(io::Error::from(error)))
}
