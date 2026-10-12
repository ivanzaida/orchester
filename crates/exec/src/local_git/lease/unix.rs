use std::{
  fs::{File, OpenOptions},
  path::Path,
};

use super::super::{LocalError, LocalGitError};

pub(super) fn acquire(path: &Path) -> Result<File, LocalGitError> {
  let file = OpenOptions::new()
    .read(true)
    .write(true)
    .create(true)
    .truncate(false)
    .open(path)
    .map_err(|_| LocalGitError::Io)?;
  rustix::fs::flock(&file, rustix::fs::FlockOperation::NonBlockingLockExclusive)
    .map_err(|_| LocalError::ForeignOperation)?;
  Ok(file)
}
