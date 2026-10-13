use std::{
  ffi::{OsStr, OsString},
  fs::File,
  io,
  os::unix::fs::{MetadataExt, PermissionsExt},
  path::{Component, Path, PathBuf},
  sync::Arc,
};

use rustix::fs::{AtFlags, CWD, Mode, OFlags, mkdirat, openat, statat};

use super::super::{LocalError, LocalGitError};

pub struct Directory {
  file: File,
  path: PathBuf,
  parent: Option<Arc<Self>>,
  name: OsString,
}

impl Directory {
  pub fn open(root: &Path) -> Result<Arc<Self>, LocalGitError> {
    if !root.is_absolute() {
      return Err(LocalError::Unsupported.into());
    }
    let fd = openat(CWD, "/", dir_flags(), Mode::empty()).map_err(map)?;
    let mut directory = Arc::new(Self {
      file: fd.into(),
      path: PathBuf::from("/"),
      parent: None,
      name: OsString::new(),
    });
    for part in root.components() {
      match part {
        Component::RootDir => {}
        Component::Normal(name) => directory = directory.child(name, false)?.ok_or(LocalGitError::Io)?,
        Component::CurDir => {}
        Component::ParentDir => directory = directory.parent.clone().ok_or(LocalError::Unsupported)?,
        _ => return Err(LocalError::Unsupported.into()),
      }
    }
    Ok(directory)
  }

  pub fn path(&self) -> &Path {
    &self.path
  }

  pub fn child(self: &Arc<Self>, name: &OsStr, create: bool) -> Result<Option<Arc<Self>>, LocalGitError> {
    validate_name(name)?;
    self.ensure_current()?;
    let fd = match openat(&self.file, name, dir_flags(), Mode::empty()) {
      Ok(fd) => fd,
      Err(rustix::io::Errno::NOENT) if create => {
        match mkdirat(&self.file, name, Mode::RWXU) {
          Ok(()) | Err(rustix::io::Errno::EXIST) => {}
          Err(error) => return Err(map(error)),
        }
        openat(&self.file, name, dir_flags(), Mode::empty()).map_err(map)?
      }
      Err(rustix::io::Errno::NOENT) => return Ok(None),
      Err(error) => return Err(map(error)),
    };
    let directory = Arc::new(Self {
      file: fd.into(),
      path: self.path.join(name),
      parent: Some(self.clone()),
      name: name.to_owned(),
    });
    directory.ensure_current()?;
    Ok(Some(directory))
  }

  pub fn ensure_current(&self) -> Result<(), LocalGitError> {
    if let Some(parent) = &self.parent {
      parent.ensure_current()?;
      parent.same_entry(&self.name, &self.file)?;
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
    let left = rustix::fs::fstat(&left).map_err(map)?;
    let right = rustix::fs::fstat(&right).map_err(map)?;
    self.ensure_current()?;
    Ok(left.st_dev == right.st_dev && left.st_ino == right.st_ino)
  }

  pub(super) fn same_entry(&self, name: &OsStr, file: &File) -> Result<(), LocalGitError> {
    let stat = statat(&self.file, name, AtFlags::SYMLINK_NOFOLLOW).map_err(map)?;
    let opened = rustix::fs::fstat(file).map_err(map)?;
    if stat.st_dev != opened.st_dev || stat.st_ino != opened.st_ino {
      return Err(LocalError::Stale.into());
    }
    Ok(())
  }

  pub(super) fn open_regular(&self, name: &OsStr, writing: bool, create: bool) -> Result<Option<File>, LocalGitError> {
    let flags = (if writing { OFlags::RDWR } else { OFlags::RDONLY })
      | OFlags::NOFOLLOW
      | OFlags::CLOEXEC
      | OFlags::NONBLOCK
      | if create {
        OFlags::CREATE | OFlags::EXCL
      } else {
        OFlags::empty()
      };
    match openat(&self.file, name, flags, Mode::RUSR | Mode::WUSR) {
      Ok(fd) => Ok(Some(fd.into())),
      Err(rustix::io::Errno::NOENT) if !create => Ok(None),
      Err(rustix::io::Errno::EXIST) => Err(LocalError::Collision.into()),
      Err(error) => Err(map(error)),
    }
  }
}

fn dir_flags() -> OFlags {
  OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::RDONLY
}

fn validate_name(name: &OsStr) -> Result<(), LocalGitError> {
  if Path::new(name).components().count() != 1
    || !matches!(Path::new(name).components().next(), Some(Component::Normal(_)))
  {
    return Err(LocalError::Unsupported.into());
  }
  Ok(())
}

fn map(error: rustix::io::Errno) -> LocalGitError {
  match error {
    rustix::io::Errno::LOOP | rustix::io::Errno::NOTDIR => LocalError::Unsupported.into(),
    rustix::io::Errno::NOENT => LocalError::Stale.into(),
    other => {
      let _error = io::Error::from(other);
      LocalGitError::Io
    }
  }
}

pub(super) fn validate_regular(file: &File) -> Result<(), LocalGitError> {
  let metadata = file.metadata().map_err(|_| LocalGitError::Io)?;
  if !metadata.is_file() || metadata.nlink() != 1 {
    return Err(LocalError::Unsupported.into());
  }
  Ok(())
}
pub(super) fn executable(file: &File) -> Result<bool, LocalGitError> {
  Ok(file.metadata().map_err(|_| LocalGitError::Io)?.mode() & 0o111 != 0)
}
pub(super) fn set_executable(file: &File, value: bool) -> Result<(), LocalGitError> {
  let mut permissions = file.metadata().map_err(|_| LocalGitError::Io)?.permissions();
  let mode = permissions.mode();
  permissions.set_mode(if value { mode | 0o111 } else { mode & !0o111 });
  file.set_permissions(permissions).map_err(|_| LocalGitError::Io)
}
pub(super) fn remove(_file: &File) -> Result<(), LocalGitError> {
  // unlinkat cannot condition deletion on the identity of this opened inode.
  Err(LocalError::ConditionalRemovalUnsupported.into())
}
