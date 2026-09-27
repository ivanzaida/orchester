//! Where Orchester keeps its application data and finds its programs. Resolved only here, so a
//! per-user fallback can be added when an installer exists (`docs/architecture/storage.md`,
//! "Storage layout").

use std::{
  env, io,
  path::{Path, PathBuf},
};

const APP_DATABASE: &str = "orchester.db";
const LOG_DIR: &str = "logs";
const RUNNER_PROGRAM: &str = "orchester-runner";

/// The application database next to the running executable.
pub fn app_database_path() -> io::Result<PathBuf> {
  Ok(app_database_beside(&env::current_exe()?))
}

/// The application database for an executable at `executable`.
pub fn app_database_beside(executable: &Path) -> PathBuf {
  executable.with_file_name(APP_DATABASE)
}

/// The folder of the application log, beside the application database.
pub fn log_dir() -> io::Result<PathBuf> {
  Ok(env::current_exe()?.with_file_name(LOG_DIR))
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
