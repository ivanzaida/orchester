//! Why a project's readiness could not be read.

use orchester_store::StoreError;
use thiserror::Error;

use crate::messages::{self, Message};

#[derive(Debug, Error)]
pub enum ReadinessError {
  #[error(transparent)]
  Store(#[from] StoreError),
  /// The project is not shared with a team server, so it has no team start.
  #[error("the project is not shared with a team")]
  NotATeamProject,
}

impl ReadinessError {
  /// What to tell the person.
  pub fn message(&self) -> Message {
    match self {
      Self::Store(error) => messages::store_error(error),
      Self::NotATeamProject => Message::new("teamFillIns.errors.notTeamProject"),
    }
  }
}
