//! SQLite opens its own handles. Hold its parent chain and regular sidecar entries against substitution first.
use std::{path::Path, sync::Arc};

#[cfg(not(windows))]
use super::LocalError;
use super::{Directory, LocalGitError};

pub struct StoreOwnership {
  _directory: Arc<Directory>,
  #[cfg(windows)]
  _files: Vec<std::fs::File>,
}
impl StoreOwnership {
  pub fn acquire(root: &Path) -> Result<Arc<Self>, LocalGitError> {
    #[cfg(windows)]
    {
      let directory = super::super::files::hold_directory(root, &[".orchester"], false)?.ok_or(LocalGitError::Io)?;
      let mut files = Vec::new();
      for name in [
        "orchester.db",
        "orchester.db-wal",
        "orchester.db-shm",
        "orchester.db-journal",
      ] {
        files.push(super::windows::pin_store_entry(
          &directory,
          name.as_ref(),
          name != "orchester.db",
        )?);
      }
      Ok(Arc::new(Self {
        _directory: directory,
        _files: files,
      }))
    }
    #[cfg(not(windows))]
    {
      let _root = root;
      // A pathname SQLite VFS cannot bind its opens to our held Unix directory/object handles.
      Err(LocalError::StoreOwnershipUnsupported.into())
    }
  }
}
