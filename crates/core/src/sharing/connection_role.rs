//! What a connection is for, as a reference to a CLI kind and a bound connection carry it.

use crate::connection::{AgentProvider, ConnectionKind};

/// What a connection is for, without any of its settings: the agent provider it runs, or GitHub. A reference to a
/// connection binds to a local connection of the same role.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ConnectionRole {
  Agent(AgentProvider),
  GitHub,
}

impl ConnectionRole {
  const ALL: [Self; 5] = [
    Self::Agent(AgentProvider::CodexCli),
    Self::Agent(AgentProvider::ClaudeCodeCli),
    Self::Agent(AgentProvider::AnthropicApi),
    Self::Agent(AgentProvider::OpenAiApi),
    Self::GitHub,
  ];

  pub const fn of(kind: &ConnectionKind) -> Self {
    match kind {
      ConnectionKind::Agent(provider) => Self::Agent(*provider),
      ConnectionKind::GitHub { .. } => Self::GitHub,
    }
  }

  /// The role as a reference carries it: the connection's kind as the project database stores it.
  pub const fn as_str(self) -> &'static str {
    match self {
      Self::Agent(AgentProvider::CodexCli) => "codex_cli",
      Self::Agent(AgentProvider::ClaudeCodeCli) => "claude_code_cli",
      Self::Agent(AgentProvider::AnthropicApi) => "anthropic_api",
      Self::Agent(AgentProvider::OpenAiApi) => "openai_api",
      Self::GitHub => "github",
    }
  }

  /// The role a reference carries as [`Self::as_str`] writes it; `None` for one this version does not know.
  pub fn parse(role: &str) -> Option<Self> {
    Self::ALL.into_iter().find(|known| known.as_str() == role)
  }
}
