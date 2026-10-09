//! Links that lead to another machine, found before a path is resolved. Resolving a path (`fs::canonicalize`) follows
//! its links, and on Windows a link whose target is a network path (`\\server\share`, `\\?\UNC\…`, a drive mapped to a
//! share) makes the system contact that server, which hands it the user's credentials (NTLM) even when the step has no
//! network. So the links on the way are read first, without following them ([`stay_local`]), and a path with a link
//! that leads anywhere but a local drive, a local volume or a relative path is refused before anything resolves it;
//! so is a path on a drive mapped to a share. Every path the file tools, the outside reads, the sources, the writable
//! folders and the skill scans resolve goes through here ([`super::canonical`]).
//!
//! Known limit: the links are read, then the path is resolved, so a link swapped in between by another program could
//! still lead elsewhere; making a symbolic link on Windows needs Developer Mode or a privilege.
//!
//! On macOS and other Unix systems a link names a path of this machine: a network share is reached only through a
//! folder it is mounted on, so there is nothing to read first. Residual risk there: an automounter folder (such as
//! `/net/<host>` where autofs is on) mounts a share when a path in it is resolved.

use std::{io, path::Path};

/// How many links a path may pass through, as the systems limit link chains.
#[cfg(windows)]
const MAX_LINKS: usize = 40;

/// Checks that `path` is not on a network drive and that no link on its way, nor any link those lead through, leads
/// elsewhere than this machine's drives; nothing is resolved through such a link. Reads only the links themselves.
///
/// # Errors
///
/// An [`io::ErrorKind::PermissionDenied`] error when the path or a link leads to another machine or a device, a link
/// or a folder on the way cannot be read, or the chain is longer than the systems follow.
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
    path::{Component, Path, PathBuf, Prefix},
  };

  use super::MAX_LINKS;
  use crate::permission::remote_drive;

  /// Where a link leads.
  enum Target {
    /// Relative to the link's folder.
    Relative(PathBuf),
    /// A local drive or volume, from its root.
    Rooted(PathBuf),
    /// Another machine, a device, or a form that is not read here.
    Elsewhere,
  }

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
          let metadata = match fs::symlink_metadata(&next) {
            Ok(metadata) => metadata,
            // What does not exist is not followed by resolving either.
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(_) => return Err(refused(&next, "cannot be read")),
          };
          if !metadata.file_type().is_symlink() {
            current = next;
            continue;
          }
          links += 1;
          if links > MAX_LINKS {
            return Err(refused(&next, "leads through too many links"));
          }
          let read = fs::read_link(&next).map_err(|_| refused(&next, "cannot be read"))?;
          let target = match target(&read) {
            Target::Relative(target) => target,
            Target::Rooted(target) => {
              current = PathBuf::new();
              target
            }
            Target::Elsewhere => return Err(refused(&next, "leads to a network or device path")),
          };
          pending.extend(parts(&target));
        }
        Some(Component::Prefix(prefix)) => {
          if let Prefix::Disk(letter) | Prefix::VerbatimDisk(letter) = prefix.kind()
            && remote_drive::is_remote(char::from(letter))
          {
            return Err(refused(&part, "is a drive mapped to a network share"));
          }
          current.push(&part);
        }
        Some(Component::RootDir) => current.push(&part),
        Some(Component::CurDir) | None => {}
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

  /// Where the link target `read` leads: only a local drive path (`X:\`, `\\?\X:\`, `\??\X:\`), a volume
  /// (`\\?\Volume{…}\`, `\??\Volume{…}\`) or a relative path without a root stays here. Any other rooted target, such
  /// as `\\server\share`, `\\?\UNC\…`, `\Device\Mup\…` or `\GLOBAL??\UNC\…`, and a drive mapped to a share lead
  /// elsewhere.
  fn target(read: &Path) -> Target {
    let text = read.to_string_lossy().replace('/', "\\");
    let local_drive = |text: &str| drive_root(text).is_some_and(|letter| !remote_drive::is_remote(letter));
    if let Some(rest) = text.strip_prefix(r"\\?\").or_else(|| text.strip_prefix(r"\??\")) {
      return if local_drive(rest) || rest.starts_with("Volume{") {
        Target::Rooted(PathBuf::from(format!(r"\\?\{rest}")))
      } else {
        Target::Elsewhere
      };
    }
    if local_drive(&text) {
      return Target::Rooted(PathBuf::from(text));
    }
    let relative = !text.starts_with('\\') && !text.split('\\').next().is_some_and(|first| first.contains(':'));
    if relative {
      Target::Relative(PathBuf::from(text))
    } else {
      Target::Elsewhere
    }
  }

  /// The drive letter of `text` when it starts with a drive's root (`X:\`).
  fn drive_root(text: &str) -> Option<char> {
    let mut chars = text.chars();
    match (chars.next(), chars.next(), chars.next()) {
      (Some(letter), Some(':'), Some('\\')) if letter.is_ascii_alphabetic() => Some(letter),
      _ => None,
    }
  }

  fn refused(link: &Path, why: &str) -> io::Error {
    io::Error::new(io::ErrorKind::PermissionDenied, format!("{} {why}", link.display()))
  }
}
