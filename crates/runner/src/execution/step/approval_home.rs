//! A dedicated Codex home for interactive App Server runs. Codex App Server cannot ignore
//! global user configuration as `codex exec` can, so sharing the regular home would expose
//! unrelated MCP servers and rules. Login to this home is explicit and separate.

use std::{
  fs, io,
  path::{Path, PathBuf},
};

use orchester_core::id::ProjectId;

pub(super) fn for_project(project: ProjectId) -> io::Result<PathBuf> {
  let home = base()?.join("OrchesterApprovalHomes").join(project.to_string());
  #[cfg(unix)]
  {
    use std::os::unix::fs::PermissionsExt;
    fs::create_dir_all(&home)?;
    fs::set_permissions(&home, fs::Permissions::from_mode(0o700))?;
  }
  #[cfg(not(unix))]
  fs::create_dir_all(&home)?;
  if fs::symlink_metadata(&home)?.file_type().is_symlink() {
    return Err(io::Error::new(
      io::ErrorKind::PermissionDenied,
      "Codex approval home is a symlink",
    ));
  }
  Ok(home)
}

#[cfg(windows)]
pub(super) fn login_command(home: &Path) -> String {
  let quoted = home.to_string_lossy().replace('\'', "''");
  format!("$env:CODEX_HOME = '{quoted}'; codex login")
}

#[cfg(target_os = "macos")]
pub(super) fn login_command(home: &Path) -> String {
  let quoted = home.to_string_lossy().replace('\'', "'\\''");
  format!("CODEX_HOME='{quoted}' codex login")
}

#[cfg(not(any(windows, target_os = "macos")))]
pub(super) fn login_command(home: &Path) -> String {
  format!("Set CODEX_HOME to {} and run codex login", home.display())
}

#[cfg(windows)]
fn base() -> io::Result<PathBuf> {
  std::env::var_os("LOCALAPPDATA")
    .map(PathBuf::from)
    .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "LOCALAPPDATA is unavailable"))
}

#[cfg(target_os = "macos")]
fn base() -> io::Result<PathBuf> {
  std::env::home_dir()
    .map(|home| home.join("Library").join("Application Support"))
    .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "home directory is unavailable"))
}

#[cfg(not(any(windows, target_os = "macos")))]
fn base() -> io::Result<PathBuf> {
  Err(io::Error::new(
    io::ErrorKind::Unsupported,
    "interactive Codex approvals require Windows or macOS",
  ))
}
