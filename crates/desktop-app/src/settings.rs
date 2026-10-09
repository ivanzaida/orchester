//! The app's settings, kept in the application database (`orchester.db`, located by
//! `orchester_store::location` like the deprecated desktop's) with Orchester's other application
//! settings: the theme, and whether agents may control the window through the app's MCP server.

use std::{
  io,
  path::{Path, PathBuf},
  sync::Arc,
};

use orchester_core::appearance::ThemeSetting;
use orchester_store::{AppDb, StoreError, location};
use parking_lot::Mutex;

#[derive(Debug, thiserror::Error)]
pub enum SettingsError {
  #[error("cannot locate the application database: {0}")]
  Location(#[source] io::Error),
  #[error("cannot open the application database {path}: {source}")]
  Open {
    path: PathBuf,
    #[source]
    source: StoreError,
  },
}

/// The application database's settings. Clones share the one connection; the window and the app's MCP
/// tools use it on the event-loop thread, for one short statement at a time.
#[derive(Clone)]
pub struct AppSettings {
  db: Arc<Mutex<AppDb>>,
}

impl AppSettings {
  /// The application database of this build.
  pub fn open_app_database() -> Result<Self, SettingsError> {
    Self::open(&location::app_database_path().map_err(SettingsError::Location)?)
  }

  pub fn open(path: &Path) -> Result<Self, SettingsError> {
    let db = AppDb::open(path).map_err(|source| SettingsError::Open {
      path: path.to_path_buf(),
      source,
    })?;
    Ok(Self {
      db: Arc::new(Mutex::new(db)),
    })
  }

  pub fn theme(&self) -> Result<ThemeSetting, StoreError> {
    self.db.lock().theme()
  }

  pub fn set_theme(&self, theme: ThemeSetting) -> Result<(), StoreError> {
    self.db.lock().set_theme(theme)
  }

  /// Whether agents may control the window; `None` until it is set, when the build decides.
  pub fn mcp_window_control(&self) -> Result<Option<bool>, StoreError> {
    self.db.lock().mcp_window_control()
  }
}
