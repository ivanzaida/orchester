//! Where Orchester's desktop apps keep their application data (`docs/architecture/storage.md`,
//! "Storage layout"), shared by the deprecated `orchester-desktop` and `orchester-app`.
//!
//! The application database and the log folder are next to the executable on Windows, where the stable
//! install relies on it, and for every build that is not inside an app bundle (development builds on
//! macOS). A macOS app bundle cannot write beside its executable, so a bundled build keeps them in the
//! user's Library instead (see [`macos`]). Beside the database, the app that has it open holds the data
//! folder's lock ([`AppLock`]): one app at a time.

mod lock;
#[cfg(target_os = "macos")]
pub mod macos;

use std::{
  env, fs, io,
  path::{Path, PathBuf},
};

pub use self::lock::{AppLock, AppLockError, LockHolder};

pub(crate) const APP_DATABASE: &str = "orchester.db";
const LOG_DIR: &str = "logs";

/// Where the application database and the application log of one build live. A plain record: any
/// pair of paths is valid.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppDataLocation {
  pub database: PathBuf,
  pub log_dir: PathBuf,
}

impl AppDataLocation {
  /// The database and the `logs` folder next to `executable`.
  pub fn beside(executable: &Path) -> Self {
    Self {
      database: app_database_beside(executable),
      log_dir: executable.with_file_name(LOG_DIR),
    }
  }
}

/// Where the running build keeps its data.
#[cfg(not(target_os = "macos"))]
pub fn app_data_location() -> io::Result<AppDataLocation> {
  Ok(AppDataLocation::beside(&env::current_exe()?))
}

/// Where the running build keeps its data: the Library for an app bundle, beside the executable
/// otherwise.
#[cfg(target_os = "macos")]
pub fn app_data_location() -> io::Result<AppDataLocation> {
  let home = env::var_os("HOME").map(PathBuf::from);
  macos::location_of(&env::current_exe()?, home.as_deref())
}

/// The application database of the running build. Its folder is created if missing (the Library
/// folder of a first start on macOS).
pub fn app_database_path() -> io::Result<PathBuf> {
  let database = app_data_location()?.database;
  if let Some(folder) = database.parent() {
    fs::create_dir_all(folder)?;
  }
  Ok(database)
}

/// The application database for an executable at `executable`.
pub fn app_database_beside(executable: &Path) -> PathBuf {
  executable.with_file_name(APP_DATABASE)
}

/// The folder of the application log of the running build.
pub fn log_dir() -> io::Result<PathBuf> {
  Ok(app_data_location()?.log_dir)
}

/// Where Orchester creates the folders of the agent accounts it makes (`docs/design/agent-accounts.md`, "Folders"):
/// `%LOCALAPPDATA%\Orchester\accounts` on Windows. It is in the applications' data, which agents' reads treat as a
/// credential location, and outside every repository; it is not beside the executable, where the stable install
/// keeps its database, because that folder is neither.
#[cfg(target_os = "windows")]
pub fn accounts_dir() -> io::Result<PathBuf> {
  let local =
    env::var_os("LOCALAPPDATA").ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "LOCALAPPDATA is not set"))?;
  Ok(PathBuf::from(local).join("Orchester").join("accounts"))
}

/// Where Orchester creates the folders of the agent accounts it makes: `~/Library/Application
/// Support/Orchester/accounts`, for every build, in the Library like a bundled build's database.
#[cfg(target_os = "macos")]
pub fn accounts_dir() -> io::Result<PathBuf> {
  let home = env::var_os("HOME").ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "HOME is not set"))?;
  Ok(PathBuf::from(home).join("Library/Application Support/Orchester/accounts"))
}

/// No other platform has an application data folder for accounts yet.
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub fn accounts_dir() -> io::Result<PathBuf> {
  Err(io::Error::new(
    io::ErrorKind::Unsupported,
    "agent account folders are not supported on this platform yet",
  ))
}
