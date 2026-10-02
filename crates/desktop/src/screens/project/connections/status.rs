//! Words and icons for connection providers and statuses. Status always shows an icon and a
//! word (`DeizF`).

use lurq::{app::ctx::Ctx, node::Element};
use orchester_core::{
  connection::{AgentConnection, AgentProvider, ConnectionStatus, EndpointPreset},
  resource::ModelId,
};

use crate::ui::{icon::Icon, notice::Notice};

/// Name of the CLI, e.g. "Codex CLI", or of the native provider, e.g. "Anthropic API (native)".
pub const fn program_key(provider: AgentProvider) -> &'static str {
  match provider {
    AgentProvider::CodexCli => "connections.provider.codex",
    AgentProvider::ClaudeCodeCli => "connections.provider.claude",
    AgentProvider::AnthropicApi => "connections.provider.anthropicApi",
    AgentProvider::OpenAiApi => "connections.provider.openaiApi",
  }
}

/// That `connection`'s CLI is too old for `model`, as a refusal recorded (`q5LzI` notice); `None`
/// when nothing says so.
pub fn cli_too_old(ctx: &Ctx, connection: &AgentConnection, model: &ModelId) -> Option<Element> {
  let too_old = connection.cannot_run(model)?;
  let cli = match connection.provider() {
    AgentProvider::CodexCli => "connections.cliTooOld.codex",
    AgentProvider::ClaudeCodeCli => "connections.cliTooOld.claude",
    // A native connection runs no CLI, so no refusal names a CLI version.
    AgentProvider::AnthropicApi | AgentProvider::OpenAiApi => return None,
  };
  let model = unbroken(model.as_str());
  let heading = ctx.t_args(
    "connections.cliTooOld.heading",
    [
      ("cli", &*ctx.t(cli)),
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

/// That `connection` is to a native agent provider, which this version runs no agent on (`q5LzI`
/// notice); `None` for an agent CLI.
pub fn native_not_available(ctx: &Ctx, connection: &AgentConnection) -> Option<Element> {
  if connection.provider().runs_agents() {
    return None;
  }
  let heading = ctx.t_args(
    "connections.nativeNotAvailable.heading",
    [
      ("connection", connection.name().as_str()),
      ("provider", &*ctx.t(program_key(connection.provider()))),
    ],
  );
  Some(
    Notice::new(Icon::Lock, heading, ctx.t("connections.nativeNotAvailable.detail"))
      .id("native-not-available")
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
    AgentProvider::AnthropicApi => "connections.provider.anthropicApiShort",
    AgentProvider::OpenAiApi => "connections.provider.openaiApiShort",
  }
}

/// The preset of a native connection's endpoint, e.g. "DeepSeek preset".
pub const fn preset_key(preset: EndpointPreset) -> &'static str {
  match preset {
    EndpointPreset::Official => "connections.native.preset.official",
    EndpointPreset::DeepSeek => "connections.native.preset.deepseek",
    EndpointPreset::Custom => "connections.native.preset.custom",
  }
}

/// The company whose models the provider runs, e.g. "OpenAI".
pub const fn vendor_key(provider: AgentProvider) -> &'static str {
  match provider {
    AgentProvider::CodexCli | AgentProvider::OpenAiApi => "connections.provider.openai",
    AgentProvider::ClaudeCodeCli | AgentProvider::AnthropicApi => "connections.provider.anthropic",
  }
}

/// The status of an agent connection: *Not available yet* for a native one, which this version
/// neither checks nor runs, else what its last check found.
pub fn agent_status_label(connection: &AgentConnection) -> (Icon, &'static str) {
  if connection.provider().runs_agents() {
    status_label(connection.status())
  } else {
    (Icon::Lock, "connections.status.notAvailable")
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
/// which Orchester never runs itself. A native connection says that this version cannot use it.
pub const fn status_detail_key(status: ConnectionStatus, provider: AgentProvider) -> &'static str {
  let sign_in = match provider {
    AgentProvider::CodexCli => "connections.detail.signInCodex",
    AgentProvider::ClaudeCodeCli => "connections.detail.signInClaude",
    // A native connection is never checked, so no status describes it.
    AgentProvider::AnthropicApi | AgentProvider::OpenAiApi => return "connections.detail.nativeNotAvailable",
  };
  match status {
    ConnectionStatus::NotChecked => "connections.detail.notChecked",
    ConnectionStatus::Connected => "connections.detail.connected",
    ConnectionStatus::SignInRequired => sign_in,
    ConnectionStatus::SignInUnknown => "connections.detail.signInUnknown",
    ConnectionStatus::ExecutableMissing => "connections.detail.executableMissing",
    ConnectionStatus::NotRecognized => "connections.detail.notRecognized",
    ConnectionStatus::CouldNotRun => "connections.detail.couldNotRun",
  }
}

/// Shown while a check runs.
pub const CHECKING: (Icon, &str) = (Icon::Loader, "connections.status.checking");
