//! A computer's reviewer choice in the team's form: the one way a computer turns the reviewer its project uses into the
//! team's, as [`TeamAgentProfile::from_local`](super::super::TeamAgentProfile::from_local) does for a profile.

use std::collections::BTreeSet;

use thiserror::Error;

use super::{ReviewerDestinations, ReviewerHost, ReviewerHostError, TeamReviewer};
use crate::{
  connection::{Connection, ConnectionKind, DecisionConnection},
  id::{ConnectionId, DecisionConnectionId},
  run::review::{ReviewerChoice, ReviewerConnectionId, default_decision_threshold},
};

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum TeamReviewerError {
  /// The reviewer's connection is not an agent connection of this computer, so it names no CLI kind.
  #[error("connection {0} is not an agent connection of this computer, so the reviewer names no CLI kind")]
  Connection(ConnectionId),
  /// The reviewer's decision connection is not one of this computer.
  #[error("decision connection {0} is not a decision connection of this computer")]
  DecisionConnection(DecisionConnectionId),
  /// The decision server's host cannot be named as a destination.
  #[error("the decision server's host cannot be a team destination: {0}")]
  Host(#[from] ReviewerHostError),
}

/// What a computer knows to turn its reviewer into the team's.
#[derive(Clone, Copy, Debug)]
pub struct TeamReviewerKnown<'a> {
  /// The connections of this computer, which tell the CLI kind a language reviewer runs.
  pub connections: &'a [Connection],
  /// The decision connections of this computer, whose servers become the team's destinations.
  pub decision_connections: &'a [DecisionConnection],
}

impl TeamReviewer {
  /// `choice` as the team holds it. A language reviewer becomes the CLI kind of its connection. A decision reviewer
  /// keeps its model and threshold (the default one when it has none) and its server becomes the destination: members'
  /// computers when it is on this computer, else exactly its host. Fails when the connection is not of this computer.
  pub fn from_local(choice: &ReviewerChoice, known: TeamReviewerKnown<'_>) -> Result<Self, TeamReviewerError> {
    match choice.connection {
      ReviewerConnectionId::Language(id) => {
        let cli = known
          .connections
          .iter()
          .find(|connection| connection.id() == id)
          .and_then(|connection| match connection.kind() {
            ConnectionKind::Agent(provider) => Some(*provider),
            ConnectionKind::GitHub { .. } => None,
          })
          .ok_or(TeamReviewerError::Connection(id))?;
        Ok(Self::Language {
          cli,
          model: choice.model.clone(),
          effort: choice.effort.clone(),
        })
      }
      ReviewerConnectionId::Decision { decision } => {
        let connection = known
          .decision_connections
          .iter()
          .find(|connection| connection.id() == decision)
          .ok_or(TeamReviewerError::DecisionConnection(decision))?;
        let destinations = if connection.is_on_this_computer() {
          ReviewerDestinations::MembersComputers
        } else {
          ReviewerDestinations::Hosts(BTreeSet::from([ReviewerHost::new(connection.base_url().host())?]))
        };
        Ok(Self::Decision {
          model: choice.model.clone(),
          lowest_threshold: choice.threshold.unwrap_or_else(default_decision_threshold),
          destinations,
        })
      }
    }
  }
}
