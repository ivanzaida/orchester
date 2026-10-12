//! No-follow, nonblocking openat prevents a concurrent link/FIFO substitution from escaping or blocking.
//! Paths below a held root are never reopened absolutely, including directory enumeration.
use std::{
  ffi::{OsStr, OsString},
  fs::{File, Metadata},
  io,
  os::fd::AsFd,
  path::Path,
};

use rustix::fs::{CWD, Dir, Mode, OFlags, openat};

use super::changed;

pub(super) fn root_part(path: &Path, parent: Option<&File>) -> io::Result<File> {
  let flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC;
  let fd = match parent {
    Some(parent) => openat(parent, path.file_name().ok_or_else(changed)?, flags, Mode::empty()),
    None => openat(CWD, path, flags, Mode::empty()),
  }
  .map_err(io::Error::from)?;
  Ok(File::from(fd))
}
pub(super) fn child(parent: &File, name: &OsStr, _: &Path) -> io::Result<File> {
  let flags = OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC;
  openat(parent, name, flags, Mode::empty())
    .map(File::from)
    .map_err(io::Error::from)
}
pub(super) fn metadata(file: &File) -> io::Result<Metadata> {
  let metadata = file.metadata()?;
  if !metadata.is_file() && !metadata.is_dir() {
    return Err(changed());
  }
  Ok(metadata)
}
pub(super) fn directory(file: &File) -> io::Result<()> {
  if file.metadata()?.is_dir() {
    Ok(())
  } else {
    Err(changed())
  }
}
pub(super) fn names(file: &File, _: &Path, maximum: usize) -> io::Result<Vec<OsString>> {
  let mut dir = Dir::read_from(file.as_fd()).map_err(io::Error::from)?;
  let mut names = Vec::new();
  for item in &mut dir {
    let item = item.map_err(io::Error::from)?;
    let bytes = item.file_name().to_bytes();
    if bytes == b"." || bytes == b".." {
      continue;
    }
    if names.len() == maximum {
      return Err(io::Error::new(io::ErrorKind::FileTooLarge, "discovery entry limit"));
    }
    use std::os::unix::ffi::OsStrExt;
    names.push(OsStr::from_bytes(bytes).to_owned());
  }
  Ok(names)
}
