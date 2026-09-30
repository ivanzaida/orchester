//! One lock file per revision, `.locks/<revision>`, locked exclusively while its index is built,
//! claimed, reused or removed, in this process and across processes (the runner builds, the desktop
//! removes). Work on other revisions never waits for it. The files stay: removing a lock file
//! another process may be about to lock would let two of them hold the "same" lock.

use std::{
  fs::{self, File, OpenOptions, TryLockError},
  path::{Path, PathBuf},
};

use orchester_core::revision::Revision;

use crate::IndexError;

const LOCKS: &str = ".locks";

/// The lock of one revision's index, held until dropped.
pub(super) struct RevisionLock {
  _file: File,
}

/// Waits for the lock of `revision` in the index folder `root`.
pub(super) fn lock(root: &Path, revision: &Revision) -> Result<RevisionLock, IndexError> {
  let (file, path) = open(root, revision)?;
  file.lock().map_err(IndexError::io("lock", path))?;
  Ok(RevisionLock { _file: file })
}

/// The lock of `revision` when no one holds it; `None` while a build or another removal does.
pub(super) fn try_lock(root: &Path, revision: &Revision) -> Result<Option<RevisionLock>, IndexError> {
  let (file, path) = open(root, revision)?;
  match file.try_lock() {
    Ok(()) => Ok(Some(RevisionLock { _file: file })),
    Err(TryLockError::WouldBlock) => Ok(None),
    Err(TryLockError::Error(error)) => Err(IndexError::io("lock", path)(error)),
  }
}

fn open(root: &Path, revision: &Revision) -> Result<(File, PathBuf), IndexError> {
  let folder = root.join(LOCKS);
  fs::create_dir_all(&folder).map_err(IndexError::io("create", &folder))?;
  let path = folder.join(revision.as_str());
  let file = OpenOptions::new()
    .create(true)
    .truncate(false)
    .write(true)
    .open(&path)
    .map_err(IndexError::io("open", &path))?;
  Ok((file, path))
}
