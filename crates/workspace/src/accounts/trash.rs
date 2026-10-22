//! Where a removed account's folder goes when the person ticks *Also move … to the Recycle Bin / Trash*, and the local
//! changes the person discards on a task's Local integration page (`tasks::local_integration`): a seam, so
//! only the installed app's window reaches the person's own Recycle Bin or Trash. A workspace starts with no trash
//! (removing with the box ticked is refused, in so many words); the app's `main` gives it the system's
//! ([`SystemTrash`]), and a QA copy or a test a folder of its own ([`FolderTrash`]).

use std::{
  fs, io,
  path::{Path, PathBuf},
  sync::Arc,
};

use parking_lot::RwLock;

/// Takes a file or a folder away where the person can still restore it; never a permanent delete.
pub trait Trash: Send + Sync {
  fn take(&self, path: &Path) -> io::Result<()>;
}

/// The system's Recycle Bin (Windows) or Trash (macOS).
pub struct SystemTrash;

impl Trash for SystemTrash {
  fn take(&self, path: &Path) -> io::Result<()> {
    orchester_exec::trash::move_to_trash(path)
  }
}

/// A folder that stands in for the trash: what it takes is moved into it, under its own name, for a QA copy or a
/// test to see, and is never the person's Recycle Bin or Trash. A name it holds already goes into a numbered folder
/// in it (`2/`, `3/`, …), so nothing it took is replaced.
pub struct FolderTrash(PathBuf);

impl FolderTrash {
  pub fn new(folder: &Path) -> Self {
    Self(folder.to_path_buf())
  }
}

impl Trash for FolderTrash {
  fn take(&self, path: &Path) -> io::Result<()> {
    let name = path
      .file_name()
      .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "a path without a name"))?;
    let place = (1_u32..)
      .map(|number| match number {
        1 => self.0.clone(),
        _ => self.0.join(number.to_string()),
      })
      .find(|place| fs::symlink_metadata(place.join(name)).is_err())
      .ok_or_else(|| io::Error::other("the trash folder has no free place"))?;
    fs::create_dir_all(&place)?;
    fs::rename(path, place.join(name))
  }
}

/// The trash a workspace and its clones share; none until one is given.
#[derive(Clone, Default)]
pub(crate) struct SharedTrash(Arc<RwLock<Option<Arc<dyn Trash>>>>);

impl SharedTrash {
  pub(crate) fn set(&self, trash: Arc<dyn Trash>) {
    *self.0.write() = Some(trash);
  }

  /// This workspace's trash, if it has one.
  pub(crate) fn get(&self) -> Option<Arc<dyn Trash>> {
    self.0.read().clone()
  }

  /// Whether this workspace has a trash.
  pub(crate) fn is_set(&self) -> bool {
    self.0.read().is_some()
  }

  /// Takes `folder` to the trash; an explicit error when this workspace has none.
  pub(crate) fn take(&self, folder: &Path) -> io::Result<()> {
    let trash = self.0.read().clone().ok_or_else(|| {
      io::Error::new(
        io::ErrorKind::Unsupported,
        "this window has no Recycle Bin or Trash to move folders to",
      )
    })?;
    trash.take(folder)
  }
}
