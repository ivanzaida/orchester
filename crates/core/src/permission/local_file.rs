//! Whether a path names a file of this machine, decided before anything is read through it. A program automation
//! names (an MCP server's, a command tool's) is checked here first: reading `\server\share\…`, `\?\UNC\…`, a
//! device path (`\.\…`), a drive mapped to a network share or a link that leads to one makes Windows contact that
//! server and hand it the user's credentials (NTLM), and whether the read succeeds tells the caller whether the
//! file exists. On macOS a path names this machine unless it lies on a mounted share, which is read as any folder.

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

/// `path` as the file system resolves it, links followed and written as a person writes it (without `\?\`), once
/// [`stays_local`] allowed it.
///
/// # Errors
///
/// [`stays_local`]'s refusal, or the system's when the path does not resolve.
pub fn resolve_local(path: &Path) -> io::Result<PathBuf> {
  platform::local_prefix(path)?;
  super::canonical::canonicalize(path).map(super::canonical::plain)
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
}

#[cfg(not(windows))]
mod platform {
  use std::{io, path::Path};

  /// A Unix path has no prefix that names another machine.
  pub(super) const fn local_prefix(_path: &Path) -> io::Result<()> {
    Ok(())
  }
}
