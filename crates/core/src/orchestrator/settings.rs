//! Which connection and model the project's Orchestrator runs on (Orchestrator settings › Profile). A project has
//! none until a person chooses them; until then the chat cannot send.

use crate::{connection::AgentProvider, id::ConnectionId, resource::ModelId};

/// Whether the Orchestrator can run on a connection of `provider`: Claude Code only. A native connection has no CLI to
/// start. Codex is held back ([`codex_held_back`]) although the runner can drive it.
pub const fn runs_orchestrator(provider: AgentProvider) -> bool {
  matches!(provider, AgentProvider::ClaudeCodeCli)
}

/// Whether a Codex connection is refused as the Orchestrator's CLI. Its `read-only` sandbox keeps writes out, not
/// reads: the agent's sandboxed shell could read `CODEX_HOME` (its sign-in), other repositories and the runner's
/// private files with live tokens, and the user's own execpolicy `allow` rules run matching commands outside the
/// sandbox without asking. Codex 0.159.2 has filesystem permission profiles with read denials, but they could not be
/// shown to work on Windows (they need the elevated sandbox) or macOS, so the Orchestrator does not run unconfined on
/// Codex. The runner has no Codex turn; the one written before the hold (driving App Server as steps do) is in the
/// branch's history for when they can be.
pub const fn codex_held_back(provider: AgentProvider) -> bool {
  matches!(provider, AgentProvider::CodexCli)
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
