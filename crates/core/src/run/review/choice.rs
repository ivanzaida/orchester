//! Which reviewer a project's auto mode uses: any of its agent connections (Claude Code, Codex, or a
//! native one on the Anthropic API) with a model of that connection and the effort when the model has
//! efforts. Until a person chooses, the reviewer is the project's first Claude Code connection with
//! [`DEFAULT_REVIEW_MODEL`], else its first connection a reviewer runs on, with that connection's
//! default model. A chosen connection that is gone, or of a kind no reviewer runs on yet (a native
//! connection on the OpenAI API), leaves auto mode unavailable with the reason; there is no silent
//! fallback to another connection.

use serde::{Deserialize, Serialize};

use crate::{
  connection::{AgentProvider, Connection, ConnectionKind},
  id::ConnectionId,
  resource::ModelId,
};

/// The reviewer's model on a Claude Code connection until a person chooses another.
pub const DEFAULT_REVIEW_MODEL: &str = "claude-haiku-4-5-20251001";

/// The reviewer a person chose: a connection, a model of it, and the effort, if the model has efforts.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewerChoice {
  pub connection: ConnectionId,
  pub model: ModelId,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub effort: Option<String>,
}

/// The settings of auto mode in a project: whether new runs use it, and the reviewer a person chose
/// (`None`: the default, [`resolve_reviewer`]).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AutoModeSettings {
  pub new_runs: bool,
  pub reviewer: Option<ReviewerChoice>,
}

/// How a reviewer runs, by its connection's kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewerKind {
  /// Claude Code in print mode ([`crate::connection::reviewer::ClaudeReviewer`]).
  ClaudeCode,
  /// `codex exec` ([`crate::connection::reviewer::CodexReviewer`]).
  Codex,
  /// One Messages request to the Anthropic API ([`crate::connection::reviewer::native_request`]).
  AnthropicApi,
}

impl ReviewerKind {
  /// The provider a review of this kind is spent on.
  pub const fn provider(self) -> AgentProvider {
    match self {
      Self::ClaudeCode => AgentProvider::ClaudeCodeCli,
      Self::Codex => AgentProvider::CodexCli,
      Self::AnthropicApi => AgentProvider::AnthropicApi,
    }
  }
}

/// Why auto mode has no reviewer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewerUnavailable {
  /// The project has no agent connection a reviewer runs on.
  NoConnection,
  /// The chosen connection is no longer in the project.
  ConnectionGone,
  /// The chosen connection is of a kind no reviewer runs on yet.
  Unsupported,
  /// The default connection names no model to review with.
  NoModel,
}

/// The reviewer resolved against the project's connections.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedReviewer<'a> {
  pub connection: &'a Connection,
  pub kind: ReviewerKind,
  pub model: ModelId,
  pub effort: Option<String>,
  /// Nobody chose it: it is the default.
  pub default: bool,
}

/// The kind of reviewer that runs on `connection`; `None` for one no reviewer runs on yet.
pub fn reviewer_kind(connection: &Connection) -> Option<ReviewerKind> {
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

/// The reviewer `settings` name among `connections`, or the default one.
pub fn resolve_reviewer<'a>(
  settings: &AutoModeSettings,
  connections: &'a [Connection],
) -> Result<ResolvedReviewer<'a>, ReviewerUnavailable> {
  if let Some(choice) = &settings.reviewer {
    let connection = connections
      .iter()
      .find(|connection| connection.id() == choice.connection)
      .ok_or(ReviewerUnavailable::ConnectionGone)?;
    let kind = reviewer_kind(connection).ok_or(ReviewerUnavailable::Unsupported)?;
    return Ok(ResolvedReviewer {
      connection,
      kind,
      model: choice.model.clone(),
      effort: choice.effort.clone(),
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
    connection,
    kind,
    model,
    effort: None,
    default: true,
  })
}

/// The model `connection` reviews with until a person chooses one: Haiku 4.5 on Claude Code, else the
/// connection's default model; `None` when it names none.
pub fn default_review_model(connection: &Connection) -> Option<ModelId> {
  match reviewer_kind(connection)? {
    ReviewerKind::ClaudeCode => {
      Some(ModelId::new(DEFAULT_REVIEW_MODEL).expect("the default review model is a valid id"))
    }
    ReviewerKind::Codex | ReviewerKind::AnthropicApi => default_model(connection),
  }
}

/// The model a connection runs when none is chosen: a native connection's default model, else the
/// first model its last check listed.
fn default_model(connection: &Connection) -> Option<ModelId> {
  if let Some(model) = connection.endpoint().api().and_then(|api| api.models().default_model()) {
    return Some(model.clone());
  }
  let agent = crate::connection::AgentConnection::try_from(connection.clone()).ok()?;
  agent.models().first().map(|model| model.id().clone())
}
