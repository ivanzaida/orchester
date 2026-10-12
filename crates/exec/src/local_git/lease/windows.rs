use std::{
  fs::{File, OpenOptions},
  os::windows::fs::OpenOptionsExt,
  path::Path,
};

use super::super::{LocalError, LocalGitError};

pub(super) fn acquire(path: &Path) -> Result<File, LocalGitError> {
  OpenOptions::new()
    .read(true)
    .write(true)
    .create(true)
    .truncate(false)
    .share_mode(0)
    .open(path)
    .map_err(|_| LocalError::ForeignOperation.into())
}
