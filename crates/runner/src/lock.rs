//! `<repo>/.orchester/runner.lock`: at most one runner per project.

use std::{
  fs::{File, OpenOptions, TryLockError},
  io,
  path::{Path, PathBuf},
};

use thiserror::Error;

const STATE_DIR: &str = ".orchester";
const LOCK_FILE: &str = "runner.lock";

#[derive(Debug, Error)]
pub enum LockError {
  #[error("{0} is not an Orchester project (no .orchester directory)")]
  NotInitialized(PathBuf),
  #[error("another runner is already running for this project (lock {0} is held)")]
  AlreadyRunning(PathBuf),
  #[error("cannot lock {path}: {source}")]
  Io { path: PathBuf, source: io::Error },
}

/// Held for the runner's lifetime. The operating system releases the lock when the file handle
/// closes, including when the runner crashes or is killed, so a stale lock cannot block the
/// next runner.
#[derive(Debug)]
pub struct RunnerLock {
  _file: File,
}

impl RunnerLock {
  pub fn acquire(project_root: &Path) -> Result<Self, LockError> {
    let state_dir = project_root.join(STATE_DIR);
    if !state_dir.is_dir() {
      return Err(LockError::NotInitialized(project_root.to_path_buf()));
    }
    let path = state_dir.join(LOCK_FILE);
    let io_error = |source| LockError::Io {
      path: path.clone(),
      source,
    };
    let file = OpenOptions::new()
      .create(true)
      .truncate(false)
      .write(true)
      .open(&path)
      .map_err(io_error)?;
    match file.try_lock() {
      Ok(()) => Ok(Self { _file: file }),
      Err(TryLockError::WouldBlock) => Err(LockError::AlreadyRunning(path)),
      Err(TryLockError::Error(source)) => Err(io_error(source)),
    }
  }
}
