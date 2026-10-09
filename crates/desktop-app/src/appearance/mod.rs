//! Which colors the window uses: dark, light, or the operating system's choice ([`ThemeSetting`]), and
//! the mode that resolved to. The setting can change while the app runs; nothing in the window offers
//! it yet, the app's MCP server does (`set_theme`).
//!
//! `System` reads the operating system's appearance when it is chosen ([`system::mode`]). lurq does not
//! report a later change of the operating system's appearance, so the window keeps the mode it read
//! until `System` is chosen again.

pub mod system;

use std::sync::Arc;

use lurq::app::theme::Theme;
use parking_lot::Mutex;

use crate::theme::{self, Mode};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ThemeSetting {
  #[default]
  Dark,
  Light,
  System,
}

impl ThemeSetting {
  pub const ALL: [Self; 3] = [Self::Dark, Self::Light, Self::System];

  /// The name the app's MCP tools use.
  pub const fn name(self) -> &'static str {
    match self {
      Self::Dark => "dark",
      Self::Light => "light",
      Self::System => "system",
    }
  }

  pub fn from_name(name: &str) -> Option<Self> {
    Self::ALL.into_iter().find(|setting| setting.name() == name)
  }

  /// The mode the setting stands for now.
  fn resolve(self) -> Mode {
    match self {
      Self::Dark => Mode::Dark,
      Self::Light => Mode::Light,
      Self::System => system::mode().unwrap_or_else(|error| {
        tracing::warn!(%error, "cannot read the system appearance; using dark");
        Mode::Dark
      }),
    }
  }
}

/// The setting and the mode it resolved to, shared by the window and the app's MCP tools. Both use it
/// on the event-loop thread.
#[derive(Clone, Debug)]
pub struct Appearance {
  current: Arc<Mutex<(ThemeSetting, Mode)>>,
}

impl Appearance {
  /// `setting`, resolved now.
  pub fn new(setting: ThemeSetting) -> Self {
    Self {
      current: Arc::new(Mutex::new((setting, setting.resolve()))),
    }
  }

  pub fn setting(&self) -> ThemeSetting {
    self.current.lock().0
  }

  pub fn mode(&self) -> Mode {
    self.current.lock().1
  }

  /// Chooses `setting` and gives `theme` its mode, which it returns.
  pub fn set(&self, theme: &Theme, setting: ThemeSetting) -> Mode {
    let mode = setting.resolve();
    *self.current.lock() = (setting, mode);
    theme::apply(theme, mode);
    mode
  }
}
