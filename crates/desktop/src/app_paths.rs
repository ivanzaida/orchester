//! Where Orchester keeps its application data. Resolved only here, so a per-user fallback can
//! be added when an installer exists (`docs/architecture/storage.md`, "Storage layout").

use std::{
  env, io,
  path::{Path, PathBuf},
};

const APP_DATABASE: &str = "orchester.db";

/// The application database next to the running executable.
pub fn app_database_path() -> io::Result<PathBuf> {
  Ok(app_database_beside(&env::current_exe()?))
}

/// The application database for an executable at `executable`.
pub fn app_database_beside(executable: &Path) -> PathBuf {
  executable.with_file_name(APP_DATABASE)
}
