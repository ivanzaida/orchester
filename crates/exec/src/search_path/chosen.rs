//! A program file a person chose instead of one found on the search path. It is examined, never
//! run: running it is the connection check's work.

use std::{
  fs, io,
  path::{Path, PathBuf},
};

use orchester_core::permission;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ChosenProgramError {
  #[error("{} is not an absolute path", .0.display())]
  NotAbsolute(PathBuf),
  #[error("{} is on another machine or a device", .0.display())]
  NotLocal(PathBuf),
  #[error("there is no file at {}", .0.display())]
  Missing(PathBuf),
  #[error("{} is not a file", .0.display())]
  NotAFile(PathBuf),
  #[error("{} is not executable", .0.display())]
  NotExecutable(PathBuf),
  #[error("cannot read {}: {source}", path.display())]
  Unreadable { path: PathBuf, source: io::Error },
}

/// `path` when it names a program file that can run: an absolute path of this machine to a file
/// (through links), which on macOS has an execute permission. The path is kept as chosen, not
/// resolved, so a link such as Homebrew's `/opt/homebrew/bin/claude` still names the program after
/// an upgrade.
pub fn chosen_program(path: &Path) -> Result<PathBuf, ChosenProgramError> {
  if !path.is_absolute() {
    return Err(ChosenProgramError::NotAbsolute(path.to_path_buf()));
  }
  // Before the file is read: reading a network path hands that server the user's credentials.
  if permission::stays_local(path).is_err() {
    return Err(ChosenProgramError::NotLocal(path.to_path_buf()));
  }
  let metadata = fs::metadata(path).map_err(|source| match source.kind() {
    io::ErrorKind::NotFound | io::ErrorKind::NotADirectory => ChosenProgramError::Missing(path.to_path_buf()),
    _ => ChosenProgramError::Unreadable {
      path: path.to_path_buf(),
      source,
    },
  })?;
  if !metadata.is_file() {
    return Err(ChosenProgramError::NotAFile(path.to_path_buf()));
  }
  if !executable(&metadata) {
    return Err(ChosenProgramError::NotExecutable(path.to_path_buf()));
  }
  Ok(path.to_path_buf())
}

#[cfg(unix)]
fn executable(metadata: &fs::Metadata) -> bool {
  use std::os::unix::fs::PermissionsExt;

  metadata.permissions().mode() & 0o111 != 0
}

/// Windows has no execute permission: whether a file runs depends on its kind, which the
/// connection check finds out by running it.
#[cfg(windows)]
fn executable(_metadata: &fs::Metadata) -> bool {
  true
}
