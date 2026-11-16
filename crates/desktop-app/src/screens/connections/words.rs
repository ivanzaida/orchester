//! What Connections calls things: each status as an icon and a word in its tone (`Status cell` `xy4z6`,
//! the states of `uBsQM` and `O1jDI`), the providers' names, and the line under a connection's name.

use lurq::app::theme::PaletteColor;
use orchester_core::connection::{ConnectionStatus, GitHubState};

use crate::{screens::project_settings::kit::Glyph, ui::icon::Icon};

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

pub(super) const GITHUB_GLYPH: Glyph = Glyph::GitHub;

pub(crate) const MCP_GLYPH: Icon = Icon::Plug;
