//! Where Orchester keeps its application data and finds its programs. Resolved only here
//! (`docs/architecture/storage.md`, "Storage layout").
//!
//! - The runner program is always next to the running executable; inside a macOS app bundle both are in
//!   `Orchester.app/Contents/MacOS/`.
//! - The application database and the log folder are next to the executable on Windows, where the stable install relies
//!   on it, and for every build that is not inside an app bundle (development builds on macOS). A macOS app bundle
//!   cannot write beside its executable, so a bundled build keeps them in the user's Library instead (see [`macos`]).

#[cfg(target_os = "macos")]
pub mod macos;

use std::{
  env, fs, io,
  path::{Path, PathBuf},
};

const APP_DATABASE: &str = "orchester.db";
const LOG_DIR: &str = "logs";
const RUNNER_PROGRAM: &str = "orchester-runner";

/// Where the application database and the application log of one build live. A plain record:
/// any pair of paths is valid.
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

/// Where the running build keeps its data: the Library for an app bundle, beside the
/// executable otherwise.
#[cfg(target_os = "macos")]
pub fn app_data_location() -> io::Result<AppDataLocation> {
  let home = env::var_os("HOME").map(PathBuf::from);
  macos::location_of(&env::current_exe()?, home.as_deref())
}

/// The application database of the running build. Its folder is created if missing (the
/// Library folder of a first start on macOS).
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

/// The runner program next to the running executable.
pub fn runner_path() -> io::Result<PathBuf> {
  Ok(runner_beside(&env::current_exe()?))
}

/// The runner program for an executable at `executable`, with this platform's extension.
pub fn runner_beside(executable: &Path) -> PathBuf {
  executable
    .with_file_name(RUNNER_PROGRAM)
    .with_extension(env::consts::EXE_EXTENSION)
}
