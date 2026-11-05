//! SQLite opens its own handles. Hold its parent chain and regular sidecar entries against substitution first.
//!
//! Windows holds them: nothing renames or replaces them until the ownership is dropped. Unix cannot hold a name, and
//! a pathname SQLite VFS cannot open through our handles, so it checks instead, through the held `.orchester`: the
//! chain to it has no link, and the database and each sidecar SQLite left are regular files of one link, read from
//! their entries without following a link. A substitution after that check is not prevented.
//!
//! Unix never opens the database or a sidecar here. A process's record locks on a file go when it closes any
//! descriptor of that file, so closing one dropped the locks of the process's own SQLite connections (ORC-630):
//! another process then took itself for the database's last connection and deleted the log and the shared memory,
//! or for its first and cut the shared memory this process still had mapped, and the next read of it ended in
//! SIGBUS. Windows locks by handle, so the handles it holds and closes take no lock of SQLite's with them.
use std::{path::Path, sync::Arc};

use super::{Directory, LocalGitError};

/// The SQLite database of the project and its sidecars, by name.
const DATABASE: &str = "orchester.db";
const SIDECARS: [&str; 3] = ["orchester.db-wal", "orchester.db-shm", "orchester.db-journal"];

pub struct StoreOwnership {
  _directory: Arc<Directory>,
  #[cfg(windows)]
  _files: Vec<std::fs::File>,
}
impl StoreOwnership {
  pub fn acquire(root: &Path) -> Result<Arc<Self>, LocalGitError> {
    let folder = root.join(".orchester");
    #[cfg(windows)]
    {
      let directory = super::super::files::hold_directory(root, &[".orchester"], false)?
        .ok_or_else(|| LocalGitError::missing(&folder))?;
      let mut files = vec![super::windows::pin_store_entry(
        &directory,
        DATABASE.as_ref(),
        super::windows::Pin::Existing,
      )?];
      for name in SIDECARS {
        files.push(super::windows::pin_store_entry(
          &directory,
          name.as_ref(),
          super::windows::Pin::Sidecar,
        )?);
      }
      Ok(Arc::new(Self {
        _directory: directory,
        _files: files,
      }))
    }
    #[cfg(unix)]
    {
      let directory = Directory::open(root)?
        .child(".orchester".as_ref(), false)?
        .ok_or_else(|| LocalGitError::missing(&folder))?;
      if !directory.regular_entry(DATABASE.as_ref())? {
        return Err(LocalGitError::missing(&folder.join(DATABASE)));
      }
      // A sidecar SQLite deleted meanwhile is simply not there: one reading of its entry decides.
      for name in SIDECARS {
        directory.regular_entry(name.as_ref())?;
      }
      Ok(Arc::new(Self { _directory: directory }))
    }
  }
}
