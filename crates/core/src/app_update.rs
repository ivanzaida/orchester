//! How the desktop app updates itself, as the person set it in Settings › Updates and the application database keeps
//! it: the release track it follows, and whether a downloaded update installs on its own.

use serde::{Deserialize, Serialize};

/// The release track an install follows. Beta gets every published release, Stable only those staff promoted to it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Channel {
  #[default]
  Stable,
  Beta,
}

impl Channel {
  pub const fn as_str(self) -> &'static str {
    match self {
      Self::Stable => "stable",
      Self::Beta => "beta",
    }
  }

  pub fn parse(text: &str) -> Option<Self> {
    [Self::Stable, Self::Beta]
      .into_iter()
      .find(|channel| channel.as_str() == text)
  }
}

/// Settings › Updates: Stable and on until a person changes them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UpdateSettings {
  pub channel: Channel,
  /// Whether a downloaded and verified update installs by itself once no run is at work; off, it waits for the
  /// person's *Install now*. The check and the download run either way.
  pub automatic: bool,
}

impl Default for UpdateSettings {
  fn default() -> Self {
    Self {
      channel: Channel::Stable,
      automatic: true,
    }
  }
}
