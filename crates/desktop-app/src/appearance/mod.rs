//! Which colors the window uses: dark, light, or the operating system's choice ([`ThemeSetting`], kept
//! in the app's settings), and the mode that resolved to. The setting can change while the app runs;
//! nothing in the window offers it yet, the app's MCP server does (`set_theme`).
//!
//! `System` uses the appearance the operating system had when the app started ([`system::mode`]):
//! lurq reports no later change of it, so the window keeps that mode until the app starts again.

pub mod system;

use std::sync::Arc;

use lurq::app::theme::Theme;
pub use orchester_core::appearance::ThemeSetting;
use orchester_store::StoreError;
use parking_lot::Mutex;

use crate::{
  settings::AppSettings,
  theme::{self, Mode},
};

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
  /// The operating system's appearance when the app started; `None` when it could not be read.
  system: Option<Mode>,
  settings: Option<AppSettings>,
}

impl Appearance {
  /// `setting`, resolved with the operating system's appearance `system`; later choices are kept in
  /// `settings` when there are any.
  pub fn new(setting: ThemeSetting, system: Option<Mode>, settings: Option<AppSettings>) -> Self {
    Self {
      current: Arc::new(Mutex::new((setting, resolve(setting, system)))),
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
    let settings = self.settings.as_ref().ok_or(AppearanceError::NoSettings)?;
    settings.set_theme(setting).map_err(AppearanceError::NotSaved)?;
    let mode = resolve(setting, self.system);
    *self.current.lock() = (setting, mode);
    theme::apply(theme, mode);
    Ok(mode)
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
