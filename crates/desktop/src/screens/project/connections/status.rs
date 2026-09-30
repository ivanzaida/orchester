//! Words and icons for connection providers and statuses. Status always shows an icon and a
//! word (`DeizF`).

use lurq::{app::ctx::Ctx, node::Element};
use orchester_core::{
  connection::{AgentConnection, AgentProvider, ConnectionStatus},
  resource::ModelId,
};

use crate::ui::{icon::Icon, notice::Notice};

/// Name of the CLI, e.g. "Codex CLI".
pub const fn program_key(provider: AgentProvider) -> &'static str {
  match provider {
    AgentProvider::CodexCli => "connections.provider.codex",
    AgentProvider::ClaudeCodeCli => "connections.provider.claude",
  }
}

/// That `connection`'s CLI is too old for `model`, as a refusal recorded (`q5LzI` notice); `None`
/// when nothing says so.
pub fn cli_too_old(ctx: &Ctx, connection: &AgentConnection, model: &ModelId) -> Option<Element> {
  let too_old = connection.cannot_run(model)?;
  let model = unbroken(model.as_str());
  let heading = ctx.t_args(
    "connections.cliTooOld.heading",
    [
      (
        "cli",
        &*ctx.t(match connection.provider() {
          AgentProvider::CodexCli => "connections.cliTooOld.codex",
          AgentProvider::ClaudeCodeCli => "connections.cliTooOld.claude",
        }),
      ),
      ("installed", too_old.installed.as_str()),
      ("model", &model),
      ("required", too_old.required.as_str()),
    ],
  );
  Some(
    Notice::new(Icon::TriangleAlert, heading, ctx.t("connections.cliTooOld.detail"))
      .id("cli-too-old")
      .into(),
  )
}

/// `id` with a word joiner (U+2060) after each hyphen, so a narrow column wraps a model id as a
/// whole instead of at a hyphen.
fn unbroken(id: &str) -> String {
  id.replace('-', "-\u{2060}")
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
