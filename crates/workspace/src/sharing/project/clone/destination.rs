//! Where a clone may go: a folder that doesn't exist yet in one that does, or an empty folder. Anything already there,
//! a hidden file or a file at the path itself, is the person's, and Git never clones over it. Read without writing,
//! and a path that names another machine is refused by its text before anything reads it, as a folder to link is.

use std::{
  fs, io,
  path::{Component, Path, PathBuf},
};

use orchester_core::permission::stays_local;

/// What a folder is, as the place for a new checkout.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Destination {
  /// It doesn't exist; Git creates it in `parent`.
  New { parent: PathBuf },
  /// An empty folder: Git clones into it.
  EmptyFolder,
  /// Something is there already: a folder with files, hidden ones too, or a file.
  Occupied,
  /// Neither it nor `parent`, the folder it would be made in, exists.
  NoParent { parent: PathBuf },
  /// The path is not absolute: it would name a folder relative to wherever Orchester runs.
  NotAbsolute,
  /// The path names another machine, a device or a drive mapped to a network share. Nothing was read through it.
  NotLocal,
  /// It, or the folder it would be made in, can't be read.
  Unreadable,
}

impl Destination {
  /// Whether Git may clone there.
  pub const fn allows(&self) -> bool {
    matches!(self, Self::New { .. } | Self::EmptyFolder)
  }
}

/// `path` without a separator at its end or `.` parts, as Git and the folder check are given it.
pub fn plain_destination(path: &Path) -> PathBuf {
  path.components().filter(|part| *part != Component::CurDir).collect()
}

/// What `path` is as the place for a new checkout. Blocking.
pub fn check_destination(path: &Path) -> Destination {
  if !path.is_absolute() {
    return Destination::NotAbsolute;
  }
  if let Err(error) = stays_local(path) {
    tracing::warn!(%error, "a folder chosen for a clone is not on this computer");
    return Destination::NotLocal;
  }
  match fs::symlink_metadata(path) {
    Ok(metadata) if metadata.is_dir() => folder(path),
    Ok(_) => Destination::Occupied,
    Err(error) if error.kind() == io::ErrorKind::NotFound => new_in(path),
    Err(error) => {
      tracing::warn!(%error, folder = %path.display(), "cannot read a folder chosen for a clone");
      Destination::Unreadable
    }
  }
}

/// An existing folder: empty, or holding anything at all.
fn folder(path: &Path) -> Destination {
  match fs::read_dir(path).map(|mut entries| entries.next()) {
    Ok(None) => Destination::EmptyFolder,
    Ok(Some(_)) => Destination::Occupied,
    Err(error) => {
      tracing::warn!(%error, folder = %path.display(), "cannot list a folder chosen for a clone");
      Destination::Unreadable
    }
  }
}

/// A path that doesn't exist: a new folder when its parent is a folder.
fn new_in(path: &Path) -> Destination {
  let Some(parent) = path.parent() else {
    return Destination::Unreadable;
  };
  match fs::metadata(parent) {
    Ok(metadata) if metadata.is_dir() => Destination::New {
      parent: parent.to_owned(),
    },
    Ok(_) => Destination::NoParent {
      parent: parent.to_owned(),
    },
    Err(error) if error.kind() == io::ErrorKind::NotFound => Destination::NoParent {
      parent: parent.to_owned(),
    },
    Err(error) => {
      tracing::warn!(%error, folder = %parent.display(), "cannot read the folder a clone would be made in");
      Destination::Unreadable
    }
  }
}
