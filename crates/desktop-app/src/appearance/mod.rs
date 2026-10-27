//! Which colors the window uses: dark, light, or the operating system's choice ([`ThemeSetting`], kept
//! in the application database through the session's workspace), and the mode that resolved to. The setting can change
//! while the app runs: in Settings › General, and through the app's MCP server (`set_theme`).
//!
//! `System` uses the appearance the operating system had when the app started ([`system::mode`]):
//! lurq reports no later change of it, so the window keeps that mode until the app starts again.

pub mod motion;
pub mod system;

use std::sync::{Arc, OnceLock};

use lurq::app::theme::Theme;
pub use orchester_core::appearance::ThemeSetting;
use orchester_store::StoreError;
use orchester_workspace::workspace::Workspace;
use parking_lot::Mutex;

use crate::theme::{self, Mode};

/// Why a theme was not chosen.
#[derive(Debug, thiserror::Error)]
pub enum AppearanceError {
  #[error("the application database is not open, so the theme cannot be kept")]
  NoSettings,
  #[error("the theme cannot be kept: {0}")]
  NotSaved(#[source] StoreError),
}

/// The setting and the mode it resolved to, shared by the window and the app's MCP tools. Both use it
/// on the event-loop thread.
#[derive(Clone)]
pub struct Appearance {
  current: Arc<Mutex<(ThemeSetting, Mode)>>,
  /// The operating system's appearance, once it has been read at start; never when it cannot be.
  system: Arc<OnceLock<Mode>>,
  /// Where the setting is kept: the session's workspace, when the session started.
  settings: Option<Workspace>,
}

impl Appearance {
  /// `setting`, resolved with the operating system's appearance `system` (filled in when it has been
  /// read); later choices are kept in `settings` when there are any.
  pub fn new(setting: ThemeSetting, system: Arc<OnceLock<Mode>>, settings: Option<Workspace>) -> Self {
    Self {
      current: Arc::new(Mutex::new((setting, resolve(setting, system.get().copied())))),
      system,
      settings,
    }
  }

  pub fn setting(&self) -> ThemeSetting {
    self.current.lock().0
  }

  pub fn mode(&self) -> Mode {
    self.current.lock().1
  }

  /// Keeps `setting` in the settings, then gives `theme` its mode, which it returns. Nothing changes
  /// when it cannot be kept.
  pub fn set(&self, theme: &Theme, setting: ThemeSetting) -> Result<Mode, AppearanceError> {
    let mode = self.keep(setting)?;
    self.show(theme, setting, mode);
    Ok(mode)
  }

  /// Keeps `setting` in the settings and returns the mode it stands for, changing nothing on screen. It blocks on
  /// SQLite: Settings › General calls it off the UI thread, then [`Self::show`] on it.
  pub fn keep(&self, setting: ThemeSetting) -> Result<Mode, AppearanceError> {
    let settings = self.settings.as_ref().ok_or(AppearanceError::NoSettings)?;
    settings.set_theme(setting).map_err(AppearanceError::NotSaved)?;
    Ok(resolve(setting, self.system.get().copied()))
  }

  /// Makes `setting`, which [`Self::keep`] kept, the current one, and gives `theme` its `mode`.
  pub fn show(&self, theme: &Theme, setting: ThemeSetting, mode: Mode) {
    *self.current.lock() = (setting, mode);
    theme::apply(theme, mode);
  }
}

/// The mode `setting` stands for, with the operating system's appearance `system`.
fn resolve(setting: ThemeSetting, system: Option<Mode>) -> Mode {
  match setting {
    ThemeSetting::Dark => Mode::Dark,
    ThemeSetting::Light => Mode::Light,
    ThemeSetting::System => system.unwrap_or_else(|| {
      tracing::warn!("the system appearance is unknown; using dark");
      Mode::Dark
    }),
  }
}
