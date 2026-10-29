//! SQLite opens its own handles. Hold its parent chain and regular sidecar entries against substitution first.
//!
//! Windows holds them: nothing renames or replaces them until the ownership is dropped. Unix cannot hold a name, and
//! a pathname SQLite VFS cannot open through our handles, so it checks instead, through no-follow handles: the chain
//! to `.orchester` has no link, and the database and each sidecar SQLite left are regular files of one link. A
//! substitution after that check is not prevented.
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
    #[cfg(windows)]
    {
      let directory = super::super::files::hold_directory(root, &[".orchester"], false)?.ok_or(LocalGitError::Io)?;
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
        .ok_or(LocalGitError::Io)?;
      if !regular(&directory, DATABASE)? {
        return Err(LocalGitError::Io);
      }
      for name in SIDECARS {
        sidecar(&directory, name)?;
      }
      Ok(Arc::new(Self { _directory: directory }))
    }
  }
}

/// Whether `name` is in `directory`; refused unless it is a regular file of one link, opened without following a link.
#[cfg(unix)]
fn regular(directory: &Directory, name: &str) -> Result<bool, LocalGitError> {
  let Some(file) = directory.open_regular(name.as_ref(), false, false)? else {
    return Ok(false);
  };
  super::unix::validate_regular(&file)?;
  directory.same_entry(name.as_ref(), &file)?;
  Ok(true)
}

/// How often a sidecar SQLite deletes or creates anew while it is checked is checked again.
#[cfg(unix)]
const SIDECAR_CHECKS: usize = 100;

/// Checks the sidecar `name` as [`regular`] does. SQLite in another connection deletes its sidecars when its last
/// connection closes and the next one creates them anew, so one deleted since it was opened here, or replaced by a
/// new one, is checked again; one that keeps changing is refused.
#[cfg(unix)]
fn sidecar(directory: &Directory, name: &str) -> Result<(), LocalGitError> {
  use std::os::unix::fs::MetadataExt;
  for _ in 0..SIDECAR_CHECKS {
    let Some(file) = directory.open_regular(name.as_ref(), false, false)? else {
      return Ok(());
    };
    // One reading decides both: SQLite may delete the sidecar between two.
    let metadata = file.metadata().map_err(|_| LocalGitError::Io)?;
    if !metadata.is_file() || metadata.nlink() > 1 {
      return Err(super::LocalError::Unsupported.into());
    }
    if metadata.nlink() == 0 {
      continue;
    }
    match directory.same_entry(name.as_ref(), &file) {
      Ok(()) => return Ok(()),
      Err(LocalGitError::State(super::LocalError::Stale)) => {}
      Err(error) => return Err(error),
    }
  }
  Err(LocalGitError::Io)
}
