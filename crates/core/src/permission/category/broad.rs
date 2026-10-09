//! Folders too broad for Approve similar to cover what lies in them (`docs/interactive-tool-approvals.md`,
//! "Orchester's tools"): a rule for one of them would approve reading far more than the person saw. Paths are compared
//! without case, so a folder written in other letters is still the same one.

use std::path::{Component, Path, PathBuf};

/// The folders users' homes lie in on Unix systems, besides the parents of the homes Orchester knows.
#[cfg(unix)]
const PROFILE_ROOTS: [&str; 2] = ["/Users", "/home"];
#[cfg(windows)]
const PROFILE_ROOTS: [&str; 0] = [];

/// Whether `folder` (resolved) is too broad: a drive or file system root, a mount point, a folder fewer than two
/// levels below one of those, a home folder of `homes` or of another user (a folder in the profiles folder, such as
/// `C:\Users` or `/Users`), a folder that holds one, or one fewer than two levels below one.
pub(super) fn is_broad(folder: &Path, homes: &[PathBuf]) -> bool {
  let key = folded(folder);
  let below = |base: &Path| {
    key
      .strip_prefix(base)
      .map_or(usize::MAX, |rest| rest.components().count())
  };
  let names = key
    .components()
    .filter(|component| matches!(component, Component::Normal(_)))
    .count();
  let homes: Vec<PathBuf> = homes.iter().map(|home| folded(home)).collect();
  // A profiles folder that is a root itself (the home `/root`) is the root, which the names already count.
  let mut profiles = homes
    .iter()
    .filter_map(|home| home.parent())
    .filter(|profiles| profiles.components().any(|part| matches!(part, Component::Normal(_))))
    .map(Path::to_path_buf)
    .chain(PROFILE_ROOTS.iter().map(|root| folded(Path::new(root))));
  names < 2
    || homes.iter().any(|home| home.starts_with(&key) || below(home) < 2)
    // `<profiles>/<user>/<folder>`: a user's home and the folders right below it.
    || profiles.any(|profiles| profiles.starts_with(&key) || below(&profiles) < 3)
    || mount_depth(folder) < 2
}

/// How many levels `folder` lies below the closest mount point at or above it, the file system's root not counted;
/// `usize::MAX` when there is none.
fn mount_depth(folder: &Path) -> usize {
  folder
    .ancestors()
    .take_while(|ancestor| ancestor.parent().is_some())
    .position(platform::is_mount)
    .unwrap_or(usize::MAX)
}

/// `path` in lower case, for comparing folders on file systems that ignore case (the safe side where they do not).
fn folded(path: &Path) -> PathBuf {
  PathBuf::from(path.to_string_lossy().to_lowercase())
}

#[cfg(unix)]
mod platform {
  use std::{fs, os::unix::fs::MetadataExt, path::Path};

  /// Whether `path` is on another device than its parent: a mounted volume. A folder that cannot be read is no
  /// mount point here; a read in it fails anyway.
  pub(super) fn is_mount(path: &Path) -> bool {
    let device = |path: &Path| fs::metadata(path).map(|metadata| metadata.dev());
    match (path.parent(), device(path)) {
      (Some(parent), Ok(own)) => device(parent).is_ok_and(|parent| parent != own),
      _ => false,
    }
  }
}

#[cfg(windows)]
mod platform {
  use std::{fs, os::windows::fs::MetadataExt, path::Path};

  const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;

  /// Whether `path`, a folder of a resolved path, is a reparse point: links and junctions were followed when it was
  /// resolved, so what is left is a volume mounted in a folder (or a cloud folder, which errs on the safe side). A
  /// folder that cannot be read is no mount point here; a read in it fails anyway.
  pub(super) fn is_mount(path: &Path) -> bool {
    fs::symlink_metadata(path)
      .is_ok_and(|metadata| metadata.is_dir() && metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0)
  }
}
