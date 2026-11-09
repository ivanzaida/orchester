//! Which connection and model the project's Orchestrator runs on (Orchestrator settings › Profile). A project has
//! none until a person chooses them; until then the chat cannot send. Beside them, how the Orchestrator watches the
//! project's runs (Project settings › Orchestrator, [`WatchSettings`]).

use thiserror::Error;

use crate::{connection::AgentProvider, id::ConnectionId, resource::ModelId};

/// Whether the Orchestrator can run on a connection of `provider`: Claude Code, or Codex through its App Server with
/// no tool of its own that reads or runs anything (`crate::connection::invocation::CodexChat`). A native connection has
/// no CLI to start.
pub const fn runs_orchestrator(provider: AgentProvider) -> bool {
  matches!(provider, AgentProvider::ClaudeCodeCli | AgentProvider::CodexCli)
}

/// The connection, model and reasoning effort the Orchestrator's next turn starts with.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrchestratorSettings {
  connection: ConnectionId,
  model: ModelId,
  /// `None` leaves the CLI's own default.
  effort: Option<String>,
}

impl OrchestratorSettings {
  pub const fn new(connection: ConnectionId, model: ModelId, effort: Option<String>) -> Self {
    Self {
      connection,
      model,
      effort,
    }
  }

  pub const fn connection(&self) -> ConnectionId {
    self.connection
  }

  pub const fn model(&self) -> &ModelId {
    &self.model
  }

  pub fn effort(&self) -> Option<&str> {
    self.effort.as_deref()
  }
}

#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
#[error(
  "a run without progress is from {min} to {max} minutes, or off",
  min = WatchSettings::MIN_NO_PROGRESS_MINUTES,
  max = WatchSettings::MAX_NO_PROGRESS_MINUTES
)]
pub struct NoProgressError {
  pub minutes: u32,
}

/// How the Orchestrator watches the project's runs (Project settings › Orchestrator): the order a /watch command
/// without text takes, and after how many minutes without progress a run is reported, or never. Neither widens what
/// agents may do, so agents may change them as a person can.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WatchSettings {
  /// Without the whitespace around it; empty when there is none.
  default_order: String,
  no_progress_minutes: Option<u32>,
}

impl WatchSettings {
  pub const MIN_NO_PROGRESS_MINUTES: u32 = 1;
  /// Four hours (`docs/design/orchestrator-watch.md` decision 8).
  pub const MAX_NO_PROGRESS_MINUTES: u32 = 4 * 60;
  pub const DEFAULT_NO_PROGRESS_MINUTES: u32 = 20;

  /// `default_order` is kept trimmed; `no_progress_minutes` is `None` for off, else from
  /// [`Self::MIN_NO_PROGRESS_MINUTES`] to [`Self::MAX_NO_PROGRESS_MINUTES`].
  pub fn new(default_order: &str, no_progress_minutes: Option<u32>) -> Result<Self, NoProgressError> {
    if let Some(minutes) = no_progress_minutes
      && !(Self::MIN_NO_PROGRESS_MINUTES..=Self::MAX_NO_PROGRESS_MINUTES).contains(&minutes)
    {
      return Err(NoProgressError { minutes });
    }
    Ok(Self {
      default_order: default_order.trim().to_owned(),
      no_progress_minutes,
    })
  }

  /// What a /watch command without text takes; empty when the project has none.
  pub fn default_order(&self) -> &str {
    &self.default_order
  }

  /// After how many minutes without progress a run is reported; `None` when it never is.
  pub const fn no_progress_minutes(&self) -> Option<u32> {
    self.no_progress_minutes
  }
}

impl Default for WatchSettings {
  /// No default order, and a run reported after 20 minutes without progress.
  fn default() -> Self {
    Self {
      default_order: String::new(),
      no_progress_minutes: Some(Self::DEFAULT_NO_PROGRESS_MINUTES),
    }
  }
}
