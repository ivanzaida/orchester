//! The glyphs that stand for a profile's connection: its CLI's, or its API's.
use orchester_core::connection::{AgentConnection, AgentProvider};

use crate::ui::icon::Icon;

/// The glyph that stands for a connection's CLI or API in a tile.
pub(super) fn provider_glyph(connection: Option<&AgentConnection>) -> Icon {
  match connection {
    Some(connection) if connection.api_provider().is_some() => Icon::Cloud,
    Some(connection) => cli_glyph(connection.provider()),
    None => Icon::Bot,
  }
}

/// The glyph that stands for an agent CLI, or a native agent's API.
pub(super) const fn cli_glyph(provider: AgentProvider) -> Icon {
  match provider {
    AgentProvider::ClaudeCodeCli => Icon::Sparkles,
    AgentProvider::CodexCli => Icon::SquareTerminal,
    AgentProvider::AnthropicApi | AgentProvider::OpenAiApi => Icon::Cpu,
  }
}
