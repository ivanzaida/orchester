//! Which connection and model the project's Orchestrator runs on (Orchestrator settings › Profile). A project has
//! none until a person chooses them; until then the chat cannot send.

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
