//! Why what uses a resource could not be read.

use orchester_core::id::TaskId;
use orchester_store::{RunStoreError, StoreError};
use thiserror::Error;

use crate::team_reads::TeamReadError;

#[derive(Debug, Error)]
pub enum UsedByError {
  /// The project does not have the resource (on the server: also an id that is no resource id, or a caller who may
  /// neither read nor archive it).
  #[error("the project has no such resource")]
  NotFound,
  /// The project is linked to a team server and the read was given no session of it.
  #[error("a team project is read in a session of its server")]
  NoTeamSession,
  /// The session is of another server or server project than the one the project is linked to.
  #[error("the session is of another team project")]
  OtherProject,
  /// The session the read was made in ended: sign-out, another account or another server.
  #[error("the session changed")]
  SessionChanged,
  /// The team server could not be reached, or refused the read; [`TeamReadError::Unavailable`] when it was not
  /// reached.
  #[error(transparent)]
  Team(TeamReadError),
  #[error(transparent)]
  Store(#[from] StoreError),
  #[error(transparent)]
  Runs(#[from] RunStoreError),
  /// A run of the project names a task the project does not have.
  #[error("task {0} of a run does not exist")]
  TaskMissing(TaskId),
  /// The blocking pool is shutting down with the window; it logged why.
  #[error("the read was interrupted")]
  Interrupted,
}

impl UsedByError {
  /// Whether the team server was not reached: the read may work again later, and no list is known.
  pub const fn is_not_reached(&self) -> bool {
    matches!(self, Self::Team(TeamReadError::Unavailable { .. }))
  }
}

impl From<TeamReadError> for UsedByError {
  fn from(error: TeamReadError) -> Self {
    match error {
      TeamReadError::NotFound => Self::NotFound,
      TeamReadError::SessionChanged => Self::SessionChanged,
      error => Self::Team(error),
    }
  }
}
