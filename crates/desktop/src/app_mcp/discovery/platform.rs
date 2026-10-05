//! Where the user's discovery folder is, and how a file only the user can read is made, per
//! platform. On Windows the folder is in the local app data folder, whose access the user, SYSTEM
//! and administrators have and files in it inherit. On macOS it is in the user's Library, the folder
//! is made `0700` and the file `0600` as they are created, before the token is written.

use std::{
  fs::{self, File},
  io,
  path::{Path, PathBuf},
};

/// The discovery folder, named apart from lurq's `lurq/mcp` and from the app's data folder.
const FOLDER: &str = "orchester-mcp";

/// `%LOCALAPPDATA%\orchester-mcp`.
#[cfg(windows)]
pub(super) fn user_folder() -> Option<PathBuf> {
  let folder = std::env::var_os("LOCALAPPDATA").map(|base| PathBuf::from(base).join(FOLDER));
  if folder.is_none() {
    tracing::error!("LOCALAPPDATA is not set; Orchester's MCP server writes no discovery file");
  }
  folder
}

/// `~/Library/Application Support/orchester-mcp`.
#[cfg(target_os = "macos")]
pub(super) fn user_folder() -> Option<PathBuf> {
  let folder = std::env::var_os("HOME").map(|home| {
    PathBuf::from(home)
      .join("Library")
      .join("Application Support")
      .join(FOLDER)
  });
  if folder.is_none() {
    tracing::error!("HOME is not set; Orchester's MCP server writes no discovery file");
  }
  folder
}

/// Orchester ships on Windows and macOS only; elsewhere the server has no discovery folder and says so.
#[cfg(not(any(windows, target_os = "macos")))]
pub(super) fn user_folder() -> Option<PathBuf> {
  tracing::error!("Orchester's MCP server has no discovery folder on this platform yet");
  None
}

#[cfg(windows)]
pub(super) fn create_private_folder(folder: &Path) -> io::Result<()> {
  fs::create_dir_all(folder)
}

#[cfg(unix)]
pub(super) fn create_private_folder(folder: &Path) -> io::Result<()> {
  use std::os::unix::fs::{DirBuilderExt, PermissionsExt};

  fs::DirBuilder::new().recursive(true).mode(0o700).create(folder)?;
  // A folder made by an earlier version, or by hand, may admit others.
  fs::set_permissions(folder, fs::Permissions::from_mode(0o700))
}

#[cfg(windows)]
pub(super) fn create_private_file(path: &Path) -> io::Result<File> {
  File::create(path)
}

#[cfg(unix)]
pub(super) fn create_private_file(path: &Path) -> io::Result<File> {
  use std::os::unix::fs::OpenOptionsExt;

  File::options().write(true).create_new(true).mode(0o600).open(path)
}
