//! Why a project's readiness could not be read.

use orchester_store::StoreError;
use thiserror::Error;

use crate::{
  messages::{self, Message},
  team_config::Unreached,
};

#[derive(Debug, Error)]
pub enum ReadinessError {
  #[error(transparent)]
  Store(#[from] StoreError),
  /// The project is not shared with a team server, so it has no team start.
  #[error("the project is not shared with a team")]
  NotATeamProject,
  /// The team's configuration is being read from its team server, which has not answered yet this session.
  #[error("the team's configuration is being read")]
  TeamReading,
  /// The team server could not be reached, and nothing of the team's configuration was read this session.
  #[error("the team server cannot be reached")]
  TeamUnreachable(Unreached),
}

impl ReadinessError {
  /// What to tell the person.
  pub fn message(&self) -> Message {
    match self {
      Self::Store(error) => messages::store_error(error),
      Self::NotATeamProject => Message::new("teamFillIns.errors.notTeamProject"),
      Self::TeamReading => Message::new("teamConfig.reading"),
      Self::TeamUnreachable(unreached) => unreached.reason.clone(),
    }
  }
}
