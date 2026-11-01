//! The project's reviewer: a language model on an agent connection, or an explicitly chosen decision model.
//! Defaults remain language reviewers. An unavailable choice never falls back to another connection.

use serde::{Deserialize, Serialize};

use super::Probability;
use crate::{
  connection::{AgentProvider, Connection, ConnectionKind, DecisionConnection},
  id::{ConnectionId, DecisionConnectionId},
  resource::ModelId,
};

/// The reviewer's model on a Claude Code connection until a person chooses another.
pub const DEFAULT_REVIEW_MODEL: &str = "claude-haiku-4-5-20251001";

/// The connection chosen for a review; decision connections cannot be used as agent connections.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ReviewerConnectionId {
  Language(ConnectionId),
  Decision { decision: DecisionConnectionId },
}

impl From<ConnectionId> for ReviewerConnectionId {
  fn from(id: ConnectionId) -> Self {
    Self::Language(id)
  }
}

impl std::fmt::Display for ReviewerConnectionId {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Language(id) => id.fmt(f),
      Self::Decision { decision } => decision.fmt(f),
    }
  }
}

/// The reviewer chosen in settings. Resolution keeps effort only for language models and threshold only for
/// decision models; a missing decision threshold means 0.80. Old language settings deserialize unchanged.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewerChoice {
  pub connection: ReviewerConnectionId,
  pub model: ModelId,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub effort: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub threshold: Option<Probability>,
}

impl ReviewerChoice {
  pub fn decision(connection: DecisionConnectionId, model: ModelId, threshold: Option<Probability>) -> Self {
    Self {
      connection: ReviewerConnectionId::Decision { decision: connection },
      model,
      effort: None,
      threshold: Some(threshold.unwrap_or_else(default_decision_threshold)),
    }
  }
}

/// The threshold used when a decision reviewer is chosen without one.
pub fn default_decision_threshold() -> Probability {
  Probability::new(0.80).expect("the default decision threshold is a probability")
}

/// Choosing a decision reviewer whose server is off this computer belongs to a person alone.
pub fn decision_reviewer_person_only(connection: &DecisionConnection) -> bool {
  !connection.is_on_this_computer()
}

/// The reviewer Manual asks before the person; none means the default language reviewer.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AutoModeSettings {
  pub reviewer: Option<ReviewerChoice>,
}

/// How a reviewer runs, by its connection's kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewerKind {
  ClaudeCode,
  Codex,
  AnthropicApi,
  Decision,
}

impl ReviewerKind {
  /// The language provider a review is spent on; decision models have no agent provider.
  pub const fn provider(self) -> Option<AgentProvider> {
    match self {
      Self::ClaudeCode => Some(AgentProvider::ClaudeCodeCli),
      Self::Codex => Some(AgentProvider::CodexCli),
      Self::AnthropicApi => Some(AgentProvider::AnthropicApi),
      Self::Decision => None,
    }
  }
}

/// Why auto mode has no reviewer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewerUnavailable {
  NoConnection,
  ConnectionGone,
  Unsupported,
  NoModel,
  /// The chosen decision server has not connected successfully, or its latest connection failed.
  NotConnected,
  /// The chosen decision model is absent from the server's latest model list.
  ModelGone,
}

/// A decision connection together with the current connection check's availability, supplied by the caller.
/// Cached model ids alone do not prove that a server is connected.
#[derive(Clone, Copy, Debug)]
pub struct DecisionReviewerConnection<'a> {
  pub connection: &'a DecisionConnection,
  pub connected: bool,
}

/// The connection of a resolved reviewer, retaining its distinct domain type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReviewerConnection<'a> {
  Language(&'a Connection),
  Decision(&'a DecisionConnection),
}

impl<'a> ReviewerConnection<'a> {
  pub const fn language(self) -> Option<&'a Connection> {
    match self {
      Self::Language(connection) => Some(connection),
      Self::Decision(_) => None,
    }
  }

  pub fn id(self) -> ReviewerConnectionId {
    match self {
      Self::Language(connection) => connection.id().into(),
      Self::Decision(connection) => ReviewerConnectionId::Decision {
        decision: connection.id(),
      },
    }
  }

  pub fn name(self) -> &'a crate::name::Name {
    match self {
      Self::Language(connection) => connection.name(),
      Self::Decision(connection) => connection.name(),
    }
  }
}

/// The reviewer resolved against the project's connections.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedReviewer<'a> {
  pub connection: ReviewerConnection<'a>,
  pub kind: ReviewerKind,
  pub model: ModelId,
  pub effort: Option<String>,
  pub threshold: Option<Probability>,
  pub default: bool,
}

/// The kind of language reviewer that runs on this agent connection.
pub fn reviewer_kind(connection: &Connection) -> Option<ReviewerKind> {
  // A reviewer CLI starts on its own sign-in; one on an API provider is not offered.
  if connection.api_provider().is_some() {
    return None;
  }
  match connection.kind() {
    ConnectionKind::Agent(AgentProvider::ClaudeCodeCli) => {
      connection.executable().is_some().then_some(ReviewerKind::ClaudeCode)
    }
    ConnectionKind::Agent(AgentProvider::CodexCli) => connection.executable().is_some().then_some(ReviewerKind::Codex),
    ConnectionKind::Agent(AgentProvider::AnthropicApi) => connection
      .endpoint()
      .api()
      .is_some()
      .then_some(ReviewerKind::AnthropicApi),
    ConnectionKind::Agent(AgentProvider::OpenAiApi) | ConnectionKind::GitHub { .. } => None,
  }
}

/// The chosen reviewer, or the default language reviewer. Decision connections are never defaults.
pub fn resolve_reviewer<'a>(
  settings: &AutoModeSettings,
  connections: &'a [Connection],
  decisions: &[DecisionReviewerConnection<'a>],
) -> Result<ResolvedReviewer<'a>, ReviewerUnavailable> {
  if let Some(choice) = &settings.reviewer {
    if let ReviewerConnectionId::Decision { decision } = choice.connection {
      return resolve_decision(choice, decision, decisions);
    }
    let connection = connections
      .iter()
      .find(|connection| ReviewerConnectionId::Language(connection.id()) == choice.connection)
      .ok_or(ReviewerUnavailable::ConnectionGone)?;
    let kind = reviewer_kind(connection).ok_or(ReviewerUnavailable::Unsupported)?;
    return Ok(ResolvedReviewer {
      connection: ReviewerConnection::Language(connection),
      kind,
      model: choice.model.clone(),
      effort: choice.effort.clone(),
      threshold: None,
      default: false,
    });
  }
  let mut usable: Vec<(&Connection, ReviewerKind)> = connections
    .iter()
    .filter_map(|connection| reviewer_kind(connection).map(|kind| (connection, kind)))
    .collect();
  usable.sort_by(|(left, _), (right, _)| left.name().as_str().cmp(right.name().as_str()));
  let (connection, kind) = usable
    .iter()
    .copied()
    .find(|(_, kind)| *kind == ReviewerKind::ClaudeCode)
    .or_else(|| usable.first().copied())
    .ok_or(ReviewerUnavailable::NoConnection)?;
  let model = default_review_model(connection).ok_or(ReviewerUnavailable::NoModel)?;
  Ok(ResolvedReviewer {
    connection: ReviewerConnection::Language(connection),
    kind,
    model,
    effort: None,
    threshold: None,
    default: true,
  })
}

fn resolve_decision<'a>(
  choice: &ReviewerChoice,
  id: DecisionConnectionId,
  connections: &[DecisionReviewerConnection<'a>],
) -> Result<ResolvedReviewer<'a>, ReviewerUnavailable> {
  let state = connections
    .iter()
    .find(|state| state.connection.id() == id)
    .ok_or(ReviewerUnavailable::ConnectionGone)?;
  if !state.connected {
    return Err(ReviewerUnavailable::NotConnected);
  }
  if !state.connection.models().contains(&choice.model) {
    return Err(ReviewerUnavailable::ModelGone);
  }
  Ok(ResolvedReviewer {
    connection: ReviewerConnection::Decision(state.connection),
    kind: ReviewerKind::Decision,
    model: choice.model.clone(),
    effort: None,
    threshold: Some(choice.threshold.unwrap_or_else(default_decision_threshold)),
    default: false,
  })
}

/// Haiku 4.5 on Claude Code, otherwise the language connection's default model.
pub fn default_review_model(connection: &Connection) -> Option<ModelId> {
  match reviewer_kind(connection)? {
    ReviewerKind::ClaudeCode => {
      Some(ModelId::new(DEFAULT_REVIEW_MODEL).expect("the default review model is a valid id"))
    }
    ReviewerKind::Codex | ReviewerKind::AnthropicApi => default_model(connection),
    ReviewerKind::Decision => None,
  }
}

fn default_model(connection: &Connection) -> Option<ModelId> {
  if let Some(model) = connection.endpoint().api().and_then(|api| api.models().default_model()) {
    return Some(model.clone());
  }
  let agent = crate::connection::AgentConnection::try_from(connection.clone()).ok()?;
  agent.models().first().map(|model| model.id().clone())
}
