//! Why a fill-in was not taken. No variant holds a secret value or a path from the person's computer.

use orchester_core::{
  id::{AgentAccountId, ConnectionId, DecisionConnectionId, ResourceId},
  resource::ResourceKind,
  sharing::ConnectionRole,
};
use orchester_store::StoreError;
use thiserror::Error;

use crate::{
  messages::{self, Message},
  team_config::Unreached,
};

#[derive(Debug, Error)]
pub enum FillInError {
  #[error(transparent)]
  Store(#[from] StoreError),
  /// Only a person does this: it widens what agents may do here. Nothing was changed.
  #[error("only a person does this; nothing was changed")]
  PersonOnly,
  /// The project is local, so there is no team configuration to fill in.
  #[error("the project is not shared with a team")]
  NotTeamProject,
  /// The resource is not one of this project's team resources of `kind`.
  #[error("resource {resource} is not a team resource of kind {kind:?} in this project")]
  NotTeamResource { resource: ResourceId, kind: ResourceKind },
  #[error("connection {0} does not exist")]
  ConnectionNotFound(ConnectionId),
  /// The connection is not one a profile of the CLI kind `expected` runs on; `None`: the kind is not known, and the
  /// connection is no agent connection.
  #[error("connection {connection} does not fit the profile's CLI kind {expected:?}")]
  ConnectionDoesNotFit {
    connection: ConnectionId,
    expected: Option<ConnectionRole>,
  },
  #[error("decision connection {0} does not exist")]
  DecisionConnectionNotFound(DecisionConnectionId),
  /// A language reviewer runs on an agent connection of its CLI kind, never on a decision connection.
  #[error("reviewer {0} is a language model, which runs on an agent connection")]
  ReviewerIsLanguage(ResourceId),
  /// A decision reviewer runs on a decision connection, never on an agent connection.
  #[error("reviewer {0} is a decision model, which runs on a decision connection")]
  ReviewerIsDecision(ResourceId),
  /// The team's reviewer does not list the decision server (`destinations`), so it is not bound to it (08 d).
  #[error("reviewer {0} does not list the decision connection's server as a destination")]
  ServerNotAllowed(ResourceId),
  /// The reviewer's newest version is not in the team's form (it names a connection, ORC-887), so what it runs on is
  /// not known.
  #[error("reviewer {0} is not in the team's form on this computer")]
  ReviewerNotTeamForm(ResourceId),
  #[error("agent account {0} is not in the list")]
  AccountNotFound(AgentAccountId),
  /// The account belongs to another CLI than the connection's, or the connection has no accounts.
  #[error("account {0} is not an account of the connection's CLI")]
  AccountOfOtherCli(AgentAccountId),
  #[error("the program path must be absolute")]
  ProgramNotAbsolute,
  #[error("there is no file at the program path")]
  ProgramNotAFile,
  /// A remote MCP server runs somewhere else, so it has no program on this computer.
  #[error("MCP server {0} is a remote server and has no program here")]
  RemoteServer(ResourceId),
  /// The team's configuration is being read from its team server, so what is named cannot be told the team's yet.
  #[error("the team's configuration is being read")]
  TeamReading,
  /// The team server could not be reached and nothing of the team's configuration was read this session.
  #[error("the team server cannot be reached")]
  TeamUnreachable(Unreached),
}

impl FillInError {
  /// A secret's value that was typed blank is refused where it is typed, before anything is sent here.
  pub fn blank_value_message() -> Message {
    Message::new("teamFillIns.errors.valueBlank")
  }

  /// What to tell the person.
  pub fn message(&self) -> Message {
    match self {
      Self::Store(error) => messages::store_error(error),
      Self::PersonOnly => Message::new("teamFillIns.errors.personOnly"),
      Self::NotTeamProject => Message::new("teamFillIns.errors.notTeamProject"),
      Self::NotTeamResource { .. } => Message::new("teamFillIns.errors.notTeamResource"),
      Self::ConnectionNotFound(_) => Message::new("teamFillIns.errors.connectionGone"),
      Self::ConnectionDoesNotFit { .. } => Message::new("teamFillIns.errors.connectionDoesNotFit"),
      Self::DecisionConnectionNotFound(_) => Message::new("teamFillIns.errors.decisionConnectionGone"),
      Self::ReviewerIsLanguage(_) => Message::new("teamFillIns.errors.reviewerIsLanguage"),
      Self::ReviewerIsDecision(_) => Message::new("teamFillIns.errors.reviewerIsDecision"),
      Self::ServerNotAllowed(_) => Message::new("teamFillIns.errors.serverNotAllowed"),
      Self::ReviewerNotTeamForm(_) => Message::new("teamFillIns.errors.reviewerNotTeamForm"),
      Self::AccountNotFound(_) => Message::new("teamFillIns.errors.accountGone"),
      Self::AccountOfOtherCli(_) => Message::new("teamFillIns.errors.accountOfOtherCli"),
      Self::ProgramNotAbsolute => Message::new("teamFillIns.errors.programNotAbsolute"),
      Self::ProgramNotAFile => Message::new("teamFillIns.errors.programNotAFile"),
      Self::RemoteServer(_) => Message::new("teamFillIns.errors.remoteServer"),
      Self::TeamReading => Message::new("teamConfig.reading"),
      Self::TeamUnreachable(unreached) => unreached.reason.clone(),
    }
  }
}
