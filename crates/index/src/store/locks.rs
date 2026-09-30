//! One lock file per revision, `.locks/<revision>`, in this process and across processes (the
//! runner builds, the desktop removes and rebuilds). It is locked exclusively while the revision's
//! index is built, rebuilt or removed, and shared while it is only read: reused by a run, or read
//! as the base of another revision's incremental build. Readers never wait for each other, and work
//! on other revisions never waits for it. The files stay: removing a lock file another process may
//! be about to lock would let two of them hold the "same" lock.

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

/// Waits for the shared lock of `revision` in the index folder `root`, to read its index.
pub(super) fn lock_shared(root: &Path, revision: &Revision) -> Result<RevisionLock, IndexError> {
  let (file, path) = open(root, revision)?;
  file.lock_shared().map_err(IndexError::io("lock", path))?;
  Ok(RevisionLock { _file: file })
}

/// The shared lock of `revision` when no one builds or removes it; `None` while someone does.
pub(super) fn try_lock_shared(root: &Path, revision: &Revision) -> Result<Option<RevisionLock>, IndexError> {
  let (file, path) = open(root, revision)?;
  match file.try_lock_shared() {
    Ok(()) => Ok(Some(RevisionLock { _file: file })),
    Err(TryLockError::WouldBlock) => Ok(None),
    Err(TryLockError::Error(error)) => Err(IndexError::io("lock", path)(error)),
  }
}

/// Waits for the exclusive lock of `revision` in the index folder `root`, to build it.
pub(super) fn lock(root: &Path, revision: &Revision) -> Result<RevisionLock, IndexError> {
  let (file, path) = open(root, revision)?;
  file.lock().map_err(IndexError::io("lock", path))?;
  Ok(RevisionLock { _file: file })
}

/// The exclusive lock of `revision` when no one holds it; `None` while a build, a reader or another
/// removal does.
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
