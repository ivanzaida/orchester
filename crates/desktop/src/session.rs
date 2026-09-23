//! One run of the application in the session log (`docs/architecture/storage.md`,
//! "Lifecycle"): started when the window opens, ended when it closes normally.

use std::path::{Path, PathBuf};

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
}

impl AppSession {
  /// Opens the application database next to the executable and records a new session.
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
    let (id, previous) = workspace.start_session().map_err(database_error)?;
    report_previous(previous);
    Ok(Self { workspace, id })
  }

  pub fn workspace(&self) -> &Workspace {
    &self.workspace
  }

  /// Records a normal close. There are no runs yet, so every normal close is clean.
  pub fn end(self) -> Result<(), StoreError> {
    self.workspace.end_session(self.id, SessionEnd::Clean)
  }
}

/// There is no recovery screen yet and no runs to recover, so an abnormal end is only logged.
fn report_previous(previous: PreviousSession) {
  match previous {
    PreviousSession::None | PreviousSession::Ended(SessionEnd::Clean | SessionEnd::RunsPaused) => {}
    PreviousSession::Ended(SessionEnd::QuitNow) => {
      tracing::warn!("the previous session ended with Quit now; no runs exist to recover");
    }
    PreviousSession::Crashed => {
      tracing::warn!("the previous session did not record its end (crash or killed process); no runs exist to recover");
    }
  }
}
