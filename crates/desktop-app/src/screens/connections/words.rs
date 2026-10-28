//! What Connections calls things: each status as an icon and a word in its tone (`Status cell` `xy4z6`,
//! the states of `uBsQM` and `O1jDI`), the providers' names, and the line under a connection's name.

use std::path::Path;

use lurq::app::{ctx::Ctx, theme::PaletteColor};
use orchester_core::connection::{AgentConnection, AgentProvider, ConnectionAccount, ConnectionStatus, GitHubState};

use crate::{
  screens::project_settings::kit::Glyph,
  ui::{agent_mark::AgentMark, icon::Icon},
};

/// The tone a status is said in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Tone {
  Success,
  Warning,
  Danger,
  /// Nothing known yet, or switched off.
  Muted,
  /// Under way.
  Busy,
}

impl Tone {
  /// The icon's color.
  pub(crate) const fn color(self) -> PaletteColor {
    match self {
      Self::Success => PaletteColor::Success,
      Self::Warning => PaletteColor::Warning,
      Self::Danger => PaletteColor::Danger,
      Self::Muted => PaletteColor::TextMuted,
      Self::Busy => PaletteColor::TextSecondary,
    }
  }

  /// The word's color: a status that asks for something is said in its tone.
  pub(super) const fn word_color(self) -> PaletteColor {
    match self {
      Self::Warning | Self::Danger => self.color(),
      Self::Success | Self::Muted | Self::Busy => PaletteColor::TextSecondary,
    }
  }
}

/// A status as the list and the inspector show it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Shown {
  pub icon: Icon,
  pub tone: Tone,
  pub key: &'static str,
}

impl Shown {
  const fn of(icon: Icon, tone: Tone, key: &'static str) -> Self {
    Self { icon, tone, key }
  }
}

/// While a check runs (`Checking…` in `uBsQM`).
pub(super) const CHECKING: Shown = Shown::of(Icon::LoaderCircle, Tone::Busy, "connections.status.checking");

/// A CLI connection on no account (its account was removed with *No account*): runs on it don't start, whatever its
/// last check said, until a person chooses one.
const NO_ACCOUNT: Shown = Shown::of(Icon::CircleAlert, Tone::Warning, "connections.status.noAccount");

/// The status of `connection`, whose last check says `status`: *No account* first for a CLI connection without one.
pub(crate) fn connection_status(connection: &AgentConnection, status: ConnectionStatus) -> Shown {
  if connection.account() == &ConnectionAccount::Unassigned {
    NO_ACCOUNT
  } else {
    agent_status(status)
  }
}

pub(crate) const fn agent_status(status: ConnectionStatus) -> Shown {
  use ConnectionStatus as S;
  use Tone as C;
  match status {
    S::NotChecked => Shown::of(Icon::Circle, C::Muted, "connections.status.notChecked"),
    S::Connected => Shown::of(Icon::CircleCheck, C::Success, "connections.status.connected"),
    S::SignInRequired => Shown::of(Icon::CircleAlert, C::Warning, "connections.status.signInRequired"),
    S::SignInUnknown => Shown::of(Icon::CircleHelp, C::Warning, "connections.status.signInUnknown"),
    S::ExecutableMissing => Shown::of(Icon::CircleAlert, C::Danger, "connections.status.notFound"),
    S::NotRecognized => Shown::of(Icon::CircleAlert, C::Danger, "connections.status.notRecognized"),
    S::CouldNotRun => Shown::of(Icon::CircleAlert, C::Danger, "connections.status.couldNotRun"),
    S::KeyMissing => Shown::of(Icon::KeyRound, C::Warning, "connections.status.keyMissing"),
    S::KeyRejected => Shown::of(Icon::CircleAlert, C::Danger, "connections.status.keyRejected"),
    // The key works; the account needs paying for (05k `EbPSP`).
    S::BillingProblem => Shown::of(Icon::CircleAlert, C::Warning, "connections.status.billingProblem"),
    S::EndpointUnreachable => Shown::of(Icon::CircleAlert, C::Danger, "connections.status.endpointUnreachable"),
  }
}

pub(super) const fn github_status(state: &GitHubState) -> Shown {
  use Tone as C;
  match state {
    GitHubState::NotChecked => Shown::of(Icon::Circle, C::Muted, "connections.status.notChecked"),
    GitHubState::SignedIn { .. } => Shown::of(Icon::CircleCheck, C::Success, "connections.github.status.signedIn"),
    GitHubState::NotSignedIn => Shown::of(Icon::CircleAlert, C::Warning, "connections.github.status.notSignedIn"),
    GitHubState::SignInUnknown => Shown::of(Icon::CircleHelp, C::Warning, "connections.status.signInUnknown"),
    GitHubState::WrongAccount { .. } => {
      Shown::of(Icon::CircleAlert, C::Warning, "connections.github.status.wrongAccount")
    }
    GitHubState::MissingScope { .. } => {
      Shown::of(Icon::CircleAlert, C::Warning, "connections.github.status.missingScope")
    }
    GitHubState::ProgramMissing => Shown::of(Icon::CircleAlert, C::Danger, "connections.github.status.notFound"),
    GitHubState::Unrecognized => Shown::of(Icon::CircleAlert, C::Danger, "connections.github.status.unrecognized"),
    GitHubState::CouldNotRun => Shown::of(Icon::CircleAlert, C::Danger, "connections.status.couldNotRun"),
  }
}

pub(super) const fn mcp_status(enabled: bool) -> Shown {
  if enabled {
    Shown::of(Icon::CircleCheck, Tone::Success, "connections.mcp.status.on")
  } else {
    Shown::of(Icon::CircleMinus, Tone::Muted, "connections.mcp.status.off")
  }
}

/// The provider's name, e.g. "Claude Code CLI".
pub(crate) const fn provider_key(provider: AgentProvider) -> &'static str {
  match provider {
    AgentProvider::CodexCli => "connections.provider.codex",
    AgentProvider::ClaudeCodeCli => "connections.provider.claude",
    AgentProvider::AnthropicApi => "connections.provider.anthropicApi",
    AgentProvider::OpenAiApi => "connections.provider.openaiApi",
  }
}

/// The company whose models the provider runs, e.g. "Anthropic".
pub(super) const fn vendor_key(provider: AgentProvider) -> &'static str {
  match provider {
    AgentProvider::CodexCli | AgentProvider::OpenAiApi => "connections.provider.openai",
    AgentProvider::ClaudeCodeCli | AgentProvider::AnthropicApi => "connections.provider.anthropic",
  }
}

/// The glyph of an agent connection (`Agent marks` `z7PmEL`): the mark of the maker whose models it runs (Claude Code,
/// Codex, the Anthropic and OpenAI APIs), or Lucide's cloud for a CLI on an API provider such as DeepSeek. GitHub shows
/// its mark and an MCP server, a program or a remote one, Lucide's plug, as `05` `QNxAo` draws them.
pub(super) fn agent_glyph(connection: &AgentConnection) -> Glyph {
  AgentMark::of_connection(connection).map_or(Glyph::Icon(Icon::Cloud), Glyph::Mark)
}

pub(super) const GITHUB_GLYPH: Glyph = Glyph::GitHub;

pub(crate) const MCP_GLYPH: Icon = Icon::Plug;

/// What the agent connection is, under its name: the vendor, the CLI and its version once checked
/// (`Anthropic · Claude Code CLI 2.1.89`), a native connection's provider and endpoint host, or the CLI and the
/// host of the API provider it runs on, after its version once checked (`Claude Code CLI 2.1.89 · api.deepseek.com`).
pub(super) fn agent_line(ctx: &Ctx, connection: &AgentConnection) -> String {
  let provider = ctx.t(provider_key(connection.provider()));
  if let Some(api) = connection.api_provider() {
    let host = api.base_url().host();
    return match checked_version(connection) {
      Some(version) => ctx.t_args(
        "connections.row.apiProviderVersion",
        [("program", &*provider), ("version", version), ("host", host)],
      ),
      None => ctx.t_args("connections.row.apiProvider", [("program", &*provider), ("host", host)]),
    }
    .to_string();
  }
  if let Some(api) = connection.api() {
    return ctx
      .t_args(
        "connections.row.native",
        [("provider", &*provider), ("host", api.base_url().host())],
      )
      .to_string();
  }
  let vendor = ctx.t(vendor_key(connection.provider()));
  match checked_version(connection) {
    Some(version) => ctx
      .t_args(
        "connections.row.agent",
        [("vendor", &*vendor), ("program", &*provider), ("version", version)],
      )
      .to_string(),
    None => ctx
      .t_args(
        "connections.row.agentUnchecked",
        [("vendor", &*vendor), ("program", &*provider)],
      )
      .to_string(),
  }
}

/// The CLI's version as its last check read it.
pub(super) fn checked_version(connection: &AgentConnection) -> Option<&str> {
  match &connection.last_check()?.result {
    orchester_core::connection::CheckResult::Available { version, .. } => Some(version),
    _ => None,
  }
}

/// The file name of `program`, as a row names an MCP program (`Program · npx`).
pub(super) fn program_name(program: &str) -> &str {
  Path::new(program)
    .file_name()
    .and_then(|name| name.to_str())
    .unwrap_or(program)
}
