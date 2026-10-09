//! Links that lead to another machine, found before a path is resolved. Resolving a path (`fs::canonicalize`) follows
//! its links, and on Windows a link or junction whose target is a network path (`\\server\share`, `\\?\UNC\…`) makes
//! the system contact that server, which hands it the user's credentials (NTLM) even when the step has no network. So
//! the links on the way are read first, without following them ([`stay_local`]), and a path with a link that leads
//! to a network or device path is refused before anything resolves it.
//!
//! On macOS and other Unix systems a link names a path of this machine: a network share is reached only through a
//! folder it is mounted on, so there is nothing to read first. Residual risk there: an automounter folder (such as
//! `/net/<host>` where autofs is on) mounts a share when a path in it is resolved.

use std::{io, path::Path};

/// How many links a path may pass through, as the systems limit link chains.
#[cfg(windows)]
const MAX_LINKS: usize = 40;

/// Checks that no link on the way of `path`, nor any link those lead through, leads to a network or device path; the
/// error names why, and nothing was resolved through such a link. Reads only the links themselves.
///
/// # Errors
///
/// An [`io::ErrorKind::PermissionDenied`] error when a link leads to another machine or a device, cannot be read, or
/// the chain is longer than the systems follow.
pub(crate) fn stay_local(path: &Path) -> io::Result<()> {
  platform::stay_local(path)
}

#[cfg(unix)]
mod platform {
  use std::{io, path::Path};

  /// A Unix link names a path of this machine: nothing to read first.
  pub(super) const fn stay_local(_path: &Path) -> io::Result<()> {
    Ok(())
  }
}

#[cfg(windows)]
mod platform {
  use std::{
    fs, io,
    path::{Component, Path, PathBuf},
  };

  use super::MAX_LINKS;

  pub(super) fn stay_local(path: &Path) -> io::Result<()> {
    // The parts still to walk, last first, so a link's target is walked in place of its name.
    let mut pending: Vec<PathBuf> = parts(path);
    let mut current = PathBuf::new();
    let mut links = 0;
    while let Some(part) = pending.pop() {
      match part.components().next() {
        Some(Component::ParentDir) => {
          current.pop();
        }
        Some(Component::Normal(_)) => {
          let next = current.join(&part);
          let Ok(metadata) = fs::symlink_metadata(&next) else {
            // What does not exist (or cannot be read) is not followed by resolving either.
            return Ok(());
          };
          if !metadata.file_type().is_symlink() {
            current = next;
            continue;
          }
          links += 1;
          let target = fs::read_link(&next).map_err(|_| refused(&next, "cannot be read"))?;
          if links > MAX_LINKS {
            return Err(refused(&next, "leads through too many links"));
          }
          if !is_local(&target) {
            return Err(refused(&next, "leads to a network or device path"));
          }
          if target.is_absolute() || target.has_root() {
            current = PathBuf::new();
          }
          pending.extend(parts(&target));
        }
        Some(Component::CurDir) | None => {}
        Some(Component::Prefix(_) | Component::RootDir) => current.push(&part),
      }
    }
    Ok(())
  }

  /// The components of `path`, last first.
  fn parts(path: &Path) -> Vec<PathBuf> {
    path
      .components()
      .rev()
      .map(|part| PathBuf::from(part.as_os_str()))
      .collect()
  }

  /// Whether a link's `target` stays on this machine: a drive path, a volume (`\\?\Volume{…}\`, a mounted volume) or a
  /// relative path; not a network path (`\\server\share`, `\\?\UNC\…`) or another device.
  fn is_local(target: &Path) -> bool {
    let text = target.to_string_lossy().replace('/', "\\");
    let verbatim = text.strip_prefix(r"\\?\").or_else(|| text.strip_prefix(r"\??\"));
    match verbatim {
      Some(rest) => is_drive(rest) || rest.starts_with("Volume{"),
      None => !text.starts_with(r"\\"),
    }
  }

  /// Whether `text` starts with a drive letter and a colon.
  fn is_drive(text: &str) -> bool {
    let mut chars = text.chars();
    matches!((chars.next(), chars.next()), (Some(letter), Some(':')) if letter.is_ascii_alphabetic())
  }

  fn refused(link: &Path, why: &str) -> io::Error {
    io::Error::new(
      io::ErrorKind::PermissionDenied,
      format!("the link {} {why}", link.display()),
    )
  }
}
