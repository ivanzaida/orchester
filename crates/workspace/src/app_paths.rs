//! Where Orchester keeps its application data and finds its programs. The data location is shared with
//! `orchester-app` and lives in `orchester_store::location`; the runner program is the desktop's own
//! and is always next to the running executable (inside a macOS app bundle both are in
//! `Orchester.app/Contents/MacOS/`).

use std::{
  env, io,
  path::{Path, PathBuf},
};

#[cfg(target_os = "macos")]
pub use orchester_store::location::macos;
pub use orchester_store::location::{
  AppDataLocation, app_data_location, app_database_beside, app_database_path, log_dir,
};

const RUNNER_PROGRAM: &str = "orchester-runner";

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
