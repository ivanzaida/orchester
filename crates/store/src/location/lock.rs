//! One Orchester app at a time per data folder: the app that opens the application database holds
//! `orchester.lock` beside it for as long as it runs, so a second app (the other desktop app, or a
//! second copy of the same one) never records a session, migrates the database or starts runners
//! there. The operating system lets go of the lock when the holder's file handle closes, also when
//! it crashes or is killed, so a stale lock cannot keep the next app out.
//!
//! Who holds the lock is written beside it, to `orchester.lock.holder`: Windows keeps every other
//! process from reading a file another process locked, so the lock file itself cannot say.

use std::{
  fs::{self, File, OpenOptions, TryLockError},
  io,
  path::{Path, PathBuf},
};

use orchester_core::time::Timestamp;
use serde::{Deserialize, Serialize};

const LOCK_FILE: &str = "orchester.lock";
const HOLDER_FILE: &str = "orchester.lock.holder";

/// The app that holds a data folder's lock: its process and when it started.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LockHolder {
  pub pid: u32,
  pub started_at: Timestamp,
}

#[derive(Debug, thiserror::Error)]
pub enum AppLockError {
  /// Another app holds the lock. `holder` is who, when its record could be read: it is written just
  /// after the lock is taken, so a second app that starts in that moment does not find it.
  #[error("another Orchester app has the data folder {folder} open")]
  InUse {
    folder: PathBuf,
    holder: Option<LockHolder>,
  },
  /// The lock or its holder's record cannot be written in `folder`: it is missing or not writable.
  #[error("cannot write to the data folder {folder}: {source}")]
  Io {
    folder: PathBuf,
    #[source]
    source: io::Error,
  },
}

/// The data folder's lock, held until it is dropped.
#[derive(Debug)]
pub struct AppLock {
  _file: File,
  holder: PathBuf,
}

impl AppLock {
  /// Takes the lock of the data folder that holds the application database `database`, for `holder`.
  pub fn acquire(database: &Path, holder: LockHolder) -> Result<Self, AppLockError> {
    let folder = database.parent().unwrap_or(Path::new("")).to_path_buf();
    let record = database.with_file_name(HOLDER_FILE);
    let io_error = |source| AppLockError::Io {
      folder: folder.clone(),
      source,
    };
    let file = OpenOptions::new()
      .create(true)
      .truncate(false)
      .write(true)
      .open(database.with_file_name(LOCK_FILE))
      .map_err(io_error)?;
    match file.try_lock() {
      Ok(()) => {}
      Err(TryLockError::WouldBlock) => {
        return Err(AppLockError::InUse {
          holder: read_holder(&record),
          folder,
        });
      }
      Err(TryLockError::Error(source)) => return Err(io_error(source)),
    }
    let text = serde_json::to_string(&holder).expect("a lock holder is plain numbers, which serialize to JSON");
    fs::write(&record, text).map_err(io_error)?;
    Ok(Self {
      _file: file,
      holder: record,
    })
  }
}

impl Drop for AppLock {
  fn drop(&mut self) {
    // The record goes before the lock does (fields drop after this), so an app that finds the lock
    // taken by the next holder reads that holder's record or none, never this one's. A record that
    // cannot be removed only says less: the next holder writes its own over it.
    let _ = fs::remove_file(&self.holder);
  }
}

/// Who holds the lock, from its record; `None` when there is none yet or it cannot be read, which
/// leaves only that the folder is in use.
fn read_holder(record: &Path) -> Option<LockHolder> {
  let text = fs::read_to_string(record).ok()?;
  serde_json::from_str(&text).ok()
}
