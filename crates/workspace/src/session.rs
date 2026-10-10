//! One run of the application in the session log (`docs/architecture/storage.md`,
//! "Lifecycle"): started when the window opens, ended when it closes normally. A session holds its
//! data folder's lock (`orchester_store::location::AppLock`) from before it opens the application
//! database until it ends, so one app at a time records sessions, migrates the database and starts
//! runners there.

use std::{
  io,
  path::{Path, PathBuf},
};

use orchester_core::time::Timestamp;
use orchester_store::{
  AppDb, PreviousSession, SessionEnd, SessionId, StoreError,
  location::{AppLock, AppLockError, LockHolder},
};
use thiserror::Error;

use crate::{
  app_paths,
  messages::{self, Message},
  workspace::Workspace,
};

#[derive(Debug, Error)]
pub enum StartupError {
  #[error("cannot locate the Orchester executable: {0}")]
  ExecutableLocation(#[source] io::Error),
  /// Another Orchester app has the data folder open; `holder` is which process, when it is known.
  #[error("another Orchester app has the data folder {folder} open")]
  InUse {
    folder: PathBuf,
    holder: Option<LockHolder>,
  },
  /// The data folder's lock cannot be written: the folder is missing or not writable.
  #[error("cannot write to the data folder {folder}: {source}")]
  DataFolder {
    folder: PathBuf,
    #[source]
    source: io::Error,
  },
  #[error("cannot open the application database {path}: {source}")]
  Database {
    path: PathBuf,
    #[source]
    source: StoreError,
  },
}

impl From<AppLockError> for StartupError {
  fn from(error: AppLockError) -> Self {
    match error {
      AppLockError::InUse { folder, holder } => Self::InUse { folder, holder },
      AppLockError::Io { folder, source } => Self::DataFolder { folder, source },
    }
  }
}

impl StartupError {
  /// What to tell the user; the error itself goes to the log.
  pub fn message(&self) -> Message {
    match self {
      Self::ExecutableLocation(_) => Message::new("errors.executableLocation"),
      Self::InUse { .. } => Message::new("errors.dataFolderInUse"),
      Self::DataFolder { .. } => Message::new("errors.dataFolderNotWritable"),
      Self::Database { source, .. } => messages::store_error(source),
    }
  }

  /// The database file or the data folder involved, if the failure got that far.
  pub fn path(&self) -> Option<&Path> {
    match self {
      Self::ExecutableLocation(_) => None,
      Self::InUse { folder, .. } | Self::DataFolder { folder, .. } => Some(folder),
      Self::Database { path, .. } => Some(path),
    }
  }
}

pub struct AppSession {
  workspace: Workspace,
  id: SessionId,
  started_at: Timestamp,
  previous: PreviousSession,
  /// When the previous session started, if there was one.
  previous_started_at: Option<Timestamp>,
  /// Held until the session ends; dropped after the end is recorded.
  lock: AppLock,
}

impl AppSession {
  /// Opens the application database of this build ([`app_paths::app_database_path`]) and records
  /// a new session.
  pub fn start() -> Result<Self, StartupError> {
    let path = app_paths::app_database_path().map_err(StartupError::ExecutableLocation)?;
    Self::start_at(&path)
  }

  /// Takes the lock of the data folder that holds the application database `path`, then opens it
  /// and records a new session. Another app that has the folder open leaves the database untouched.
  pub fn start_at(path: &Path) -> Result<Self, StartupError> {
    let started_at = Timestamp::now();
    let lock = AppLock::acquire(path, LockHolder::running(std::process::id(), started_at))?;
    let database_error = |source| StartupError::Database {
      path: path.to_path_buf(),
      source,
    };
    let workspace = Workspace::new(AppDb::open(path).map_err(database_error)?);
    let (id, previous) = workspace.start_session(started_at).map_err(database_error)?;
    let previous_started_at = workspace.session_started_before(id).map_err(database_error)?;
    if matches!(
      previous,
      PreviousSession::Crashed | PreviousSession::Ended(SessionEnd::QuitNow)
    ) {
      tracing::warn!(?previous, "the previous session ended before agents could save");
    }
    Ok(Self {
      workspace,
      id,
      started_at,
      previous,
      previous_started_at,
      lock,
    })
  }

  pub fn workspace(&self) -> &Workspace {
    &self.workspace
  }

  /// When this session started: runs its runners record as interrupted when they attach were
  /// cut off by the previous session's end.
  pub fn started_at(&self) -> Timestamp {
    self.started_at
  }

  /// Since when the runs the previous session left interrupted were stopped: its start. A runner can
  /// record its runs as interrupted before this session starts (on macOS it ends them itself when its
  /// app quits or goes away), so the recovery looks from there. This session's start when there was
  /// none before.
  pub fn interrupted_since(&self) -> Timestamp {
    self.previous_started_at.unwrap_or(self.started_at)
  }

  /// How the previous session ended: after Quit now or a crash, runs it left at work are
  /// offered for recovery (02.E `o8DCd`).
  pub fn previous(&self) -> PreviousSession {
    self.previous
  }

  /// How the previous session ended, as the runners are told when they attach.
  pub fn previous_end(&self) -> orchester_ipc::SessionEnd {
    match self.previous {
      PreviousSession::None | PreviousSession::Ended(SessionEnd::Clean) => orchester_ipc::SessionEnd::Clean,
      PreviousSession::Ended(SessionEnd::RunsPaused) => orchester_ipc::SessionEnd::RunsPaused,
      PreviousSession::Ended(SessionEnd::QuitNow) => orchester_ipc::SessionEnd::QuitNow,
      PreviousSession::Crashed => orchester_ipc::SessionEnd::Crashed,
    }
  }

  /// The window closed and the session is ending: an app that starts meanwhile is told this one is
  /// closing, not that it is stuck. Losing the mark only words that app's screen less precisely.
  pub fn mark_closing(&mut self) {
    if let Err(error) = self.lock.mark_closing() {
      tracing::warn!(%error, "cannot mark the data folder's lock as closing");
    }
  }

  /// Records how the window closed (see `close::session_end`).
  pub fn end(self, end: SessionEnd) -> Result<(), StoreError> {
    self.workspace.end_session(self.id, end)
  }
}
