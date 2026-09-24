//! Words and icons for connection providers and statuses. Status always shows an icon and a
//! word (`DeizF`).

use orchester_core::connection::{AgentProvider, ConnectionStatus};

use crate::ui::icon::Icon;

/// Name of the CLI, e.g. "Codex CLI".
pub const fn program_key(provider: AgentProvider) -> &'static str {
  match provider {
    AgentProvider::CodexCli => "connections.provider.codex",
    AgentProvider::ClaudeCodeCli => "connections.provider.claude",
  }
}

/// Short provider name used in a suggested connection name, e.g. "Codex".
pub const fn short_key(provider: AgentProvider) -> &'static str {
  match provider {
    AgentProvider::CodexCli => "connections.provider.codexShort",
    AgentProvider::ClaudeCodeCli => "connections.provider.claudeShort",
  }
}

/// The company whose models the CLI runs, e.g. "OpenAI".
pub const fn vendor_key(provider: AgentProvider) -> &'static str {
  match provider {
    AgentProvider::CodexCli => "connections.provider.openai",
    AgentProvider::ClaudeCodeCli => "connections.provider.anthropic",
  }
}

pub const fn status_label(status: ConnectionStatus) -> (Icon, &'static str) {
  match status {
    ConnectionStatus::NotChecked => (Icon::Circle, "connections.status.notChecked"),
    ConnectionStatus::Connected => (Icon::CircleCheck, "connections.status.connected"),
    ConnectionStatus::SignInRequired => (Icon::AlertCircle, "connections.status.signInRequired"),
    ConnectionStatus::SignInUnknown => (Icon::CircleHelp, "connections.status.signInUnknown"),
    ConnectionStatus::ExecutableMissing => (Icon::AlertCircle, "connections.status.executableMissing"),
    ConnectionStatus::NotRecognized => (Icon::AlertCircle, "connections.status.notRecognized"),
    ConnectionStatus::CouldNotRun => (Icon::AlertCircle, "connections.status.couldNotRun"),
  }
}

/// What the status means and what to do about it. Signing in names the CLI's own command,
/// which Orchester never runs itself.
pub const fn status_detail_key(status: ConnectionStatus, provider: AgentProvider) -> &'static str {
  match status {
    ConnectionStatus::NotChecked => "connections.detail.notChecked",
    ConnectionStatus::Connected => "connections.detail.connected",
    ConnectionStatus::SignInRequired => match provider {
      AgentProvider::CodexCli => "connections.detail.signInCodex",
      AgentProvider::ClaudeCodeCli => "connections.detail.signInClaude",
    },
    ConnectionStatus::SignInUnknown => "connections.detail.signInUnknown",
    ConnectionStatus::ExecutableMissing => "connections.detail.executableMissing",
    ConnectionStatus::NotRecognized => "connections.detail.notRecognized",
    ConnectionStatus::CouldNotRun => "connections.detail.couldNotRun",
  }
}

/// Shown while a check runs.
pub const CHECKING: (Icon, &str) = (Icon::Loader, "connections.status.checking");
