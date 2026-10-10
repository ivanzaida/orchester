//! Whether a path names a file of this machine, decided before anything is read through it, and where a stored
//! program leads. A program automation or a person names (an MCP server's, a command tool's) is checked here first:
//! reading `\\server\share\…`, `\\?\UNC\…`, a device path (`\\.\…`), a drive mapped to a network share or a link that
//! leads to one makes Windows contact that server and hand it the user's credentials (NTLM), and whether the read
//! succeeds tells the caller whether the file exists. On macOS a path names this machine unless it lies on a mounted
//! share, which is read as any folder.
//!
//! A program is stored as the file found (a virtual environment's `python`, a version manager's shim, a Homebrew or
//! WinGet link keep working when what they lead to moves), and where it leads is checked when it is saved and again
//! each time it starts ([`program_place`]): on this machine, and outside the project's repository, which holds the
//! worktrees agents write.

use std::{
  io,
  path::{Path, PathBuf},
};

/// Checks `path` by its text, and the links on its way without following them ([`super::link_targets`]).
///
/// # Errors
///
/// An [`io::ErrorKind::PermissionDenied`] error when the path or a link on its way leads to another machine or a
/// device.
pub fn stays_local(path: &Path) -> io::Result<()> {
  platform::local_prefix(path)?;
  super::link_targets::stay_local(path)
}

/// `path` as the file system resolves it, links followed and written as a person writes it (without `\\?\`), once
/// [`stays_local`] allowed it.
///
/// # Errors
///
/// [`stays_local`]'s refusal, or the system's when the path does not resolve.
pub fn resolve_local(path: &Path) -> io::Result<PathBuf> {
  platform::local_prefix(path)?;
  super::canonical::canonicalize(path).map(super::canonical::plain)
}

/// Why a program cannot start from where it leads.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProgramPlace {
  /// It, or a link on its way, leads to another machine or a device, or it cannot be resolved.
  NotLocal,
  /// It leads into the project's repository (its worktrees included), at this file.
  InRepository(PathBuf),
}

/// Checks that the program file `found` lies, and leads, to a file of this machine outside `repository`. A Windows App
/// Execution Alias (`%LOCALAPPDATA%\Microsoft\WindowsApps\*.exe`, which the system makes for a Store app) cannot be
/// read as a link; it is the system's own and leads into the system's package folder, so it is taken as it is.
///
/// # Errors
///
/// A [`ProgramPlace`] says where it leads instead.
pub fn program_place(found: &Path, repository: &Path) -> Result<(), ProgramPlace> {
  if platform::app_execution_alias(found) {
    return Ok(());
  }
  let target = resolve_local(found).map_err(|_| ProgramPlace::NotLocal)?;
  let repository = resolve_local(repository).map_err(|_| ProgramPlace::NotLocal)?;
  if target.starts_with(&repository) {
    return Err(ProgramPlace::InRepository(target));
  }
  // A link inside the repository that leads out of it could be pointed elsewhere by an agent.
  let folder = found.parent().ok_or(ProgramPlace::NotLocal)?;
  if resolve_local(folder)
    .map_err(|_| ProgramPlace::NotLocal)?
    .starts_with(&repository)
  {
    return Err(ProgramPlace::InRepository(found.to_path_buf()));
  }
  Ok(())
}

#[cfg(windows)]
mod platform {
  use std::{
    io,
    path::{Component, Path, Prefix},
  };

  /// Refuses a path whose prefix names another machine or a device: only a drive letter is local.
  pub(super) fn local_prefix(path: &Path) -> io::Result<()> {
    match path.components().next() {
      Some(Component::Prefix(prefix)) if !matches!(prefix.kind(), Prefix::Disk(_) | Prefix::VerbatimDisk(_)) => {
        Err(io::Error::new(
          io::ErrorKind::PermissionDenied,
          format!("{} names a network or device path", path.display()),
        ))
      }
      _ => Ok(()),
    }
  }

  /// Whether `path` is a file directly in the user's `%LOCALAPPDATA%\Microsoft\WindowsApps`, where the system puts
  /// App Execution Aliases.
  pub(super) fn app_execution_alias(path: &Path) -> bool {
    let Some(local) = std::env::var_os("LOCALAPPDATA") else {
      return false;
    };
    let aliases = Path::new(&local).join("Microsoft").join("WindowsApps");
    let same = |left: &Path, right: &Path| {
      left.as_os_str().to_string_lossy().to_lowercase() == right.as_os_str().to_string_lossy().to_lowercase()
    };
    local_prefix(&aliases).is_ok() && path.parent().is_some_and(|folder| same(folder, &aliases))
  }
}

#[cfg(not(windows))]
mod platform {
  use std::{io, path::Path};

  /// A Unix path has no prefix that names another machine.
  pub(super) const fn local_prefix(_path: &Path) -> io::Result<()> {
    Ok(())
  }

  /// Only Windows has App Execution Aliases.
  pub(super) const fn app_execution_alias(_path: &Path) -> bool {
    false
  }
}
