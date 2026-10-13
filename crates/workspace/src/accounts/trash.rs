//! Where a removed account's folder goes when the person ticks *Also move … to the Recycle Bin / Trash*: a seam, so
//! only the installed app's window reaches the person's own Recycle Bin or Trash. A workspace starts with no trash
//! (removing with the box ticked is refused, in so many words); the app's `main` gives it the system's
//! ([`SystemTrash`]), and a QA copy or a test a folder of its own ([`FolderTrash`]).

use std::{
  fs, io,
  path::{Path, PathBuf},
  sync::Arc,
};

use parking_lot::RwLock;

/// Takes a folder away where the person can still restore it; never a permanent delete.
pub trait Trash: Send + Sync {
  fn take(&self, folder: &Path) -> io::Result<()>;
}

/// The system's Recycle Bin (Windows) or Trash (macOS).
pub struct SystemTrash;

impl Trash for SystemTrash {
  fn take(&self, folder: &Path) -> io::Result<()> {
    orchester_exec::trash::move_to_trash(folder)
  }
}

/// A folder that stands in for the trash: what it takes is moved into it, under its own name, for a QA copy or a
/// test to see, and is never the person's Recycle Bin or Trash.
pub struct FolderTrash(PathBuf);

impl FolderTrash {
  pub fn new(folder: &Path) -> Self {
    Self(folder.to_path_buf())
  }
}

impl Trash for FolderTrash {
  fn take(&self, folder: &Path) -> io::Result<()> {
    let name = folder
      .file_name()
      .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "a folder without a name"))?;
    fs::create_dir_all(&self.0)?;
    fs::rename(folder, self.0.join(name))
  }
}

/// The trash a workspace and its clones share; none until one is given.
#[derive(Clone, Default)]
pub(crate) struct SharedTrash(Arc<RwLock<Option<Arc<dyn Trash>>>>);

impl SharedTrash {
  pub(crate) fn set(&self, trash: Arc<dyn Trash>) {
    *self.0.write() = Some(trash);
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
