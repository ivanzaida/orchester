//! One run of the application in the session log (`docs/architecture/storage.md`,
//! "Lifecycle"): started when the window opens, ended when it closes normally.

use std::path::{Path, PathBuf};

use orchester_core::time::Timestamp;
use orchester_store::{AppDb, PreviousSession, SessionEnd, SessionId, StoreError};
use thiserror::Error;

use crate::{
  app_paths,
  messages::{self, Message},
  workspace::Workspace,
};

#[derive(Debug, Error)]
pub enum StartupError {
  #[error("cannot locate the Orchester executable: {0}")]
  ExecutableLocation(#[source] std::io::Error),
  #[error("cannot open the application database {path}: {source}")]
  Database {
    path: PathBuf,
    #[source]
    source: StoreError,
  },
}

impl StartupError {
  /// What to tell the user; the error itself goes to the log.
  pub fn message(&self) -> Message {
    match self {
      Self::ExecutableLocation(_) => Message::new("errors.executableLocation"),
      Self::Database { source, .. } => messages::store_error(source),
    }
  }

  /// The database file involved, if the failure got that far.
  pub fn path(&self) -> Option<&Path> {
    match self {
      Self::ExecutableLocation(_) => None,
      Self::Database { path, .. } => Some(path),
    }
  }
}

pub struct AppSession {
  workspace: Workspace,
  id: SessionId,
  started_at: Timestamp,
  previous: PreviousSession,
}

impl AppSession {
  /// Opens the application database of this build ([`app_paths::app_database_path`]) and records
  /// a new session.
  pub fn start() -> Result<Self, StartupError> {
    let path = app_paths::app_database_path().map_err(StartupError::ExecutableLocation)?;
    Self::start_at(&path)
  }

  pub fn start_at(path: &Path) -> Result<Self, StartupError> {
    let database_error = |source| StartupError::Database {
      path: path.to_path_buf(),
      source,
    };
    let workspace = Workspace::new(AppDb::open(path).map_err(database_error)?);
    let started_at = Timestamp::now();
    let (id, previous) = workspace.start_session(started_at).map_err(database_error)?;
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

  /// Records how the window closed (see `close::session_end`).
  pub fn end(self, end: SessionEnd) -> Result<(), StoreError> {
    self.workspace.end_session(self.id, end)
  }
}
