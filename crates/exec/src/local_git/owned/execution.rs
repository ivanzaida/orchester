//! A checkout's execution roots, held while Git uses pathname APIs: the checkout and its `.git` entry, and the
//! scratch folder, kept nonempty by an anchor because NTFS refuses reparse conversion of a nonempty directory.
//! Opening a checkout writes nothing into its `.git`: reading changes nothing there. An operation that changes the
//! repository holds its metadata too ([`super::MetadataOwnership`]).
use std::{ffi::OsStr, fs::File, path::Path, sync::Arc};

use super::{
  Directory, LocalError, LocalGitError,
  windows::{self, Pin},
};

pub struct ExecutionOwnership {
  directories: Vec<Arc<Directory>>,
  files: Vec<File>,
}
impl ExecutionOwnership {
  pub fn acquire(root: &Arc<Directory>, scratch: &Arc<Directory>) -> Result<Arc<Self>, LocalGitError> {
    let mut directories = vec![root.clone(), scratch.clone()];
    let mut files = vec![windows::pin_store_entry(
      scratch,
      OsStr::new(".orchester-local-owner"),
      Pin::Anchor,
    )?];
    match root.child(OsStr::new(".git"), false) {
      // The held directory cannot be renamed; it is never empty, so it cannot be converted either.
      Ok(Some(git)) => directories.push(git),
      Err(LocalGitError::State(LocalError::Unsupported)) => {
        // A linked worktree has a regular .git pointer. It remains read-only and cannot be replaced.
        files.push(
          root
            .open_regular(OsStr::new(".git"), false, false)?
            .ok_or(LocalGitError::Io)?,
        );
      }
      Ok(None) => return Err(LocalError::Unsupported.into()),
      Err(error) => return Err(error),
    }
    let ownership = Arc::new(Self { directories, files });
    ownership.ensure_current()?;
    Ok(ownership)
  }
  pub fn private_copy(root: &Path, scratch: &Path) -> Result<Arc<Self>, LocalGitError> {
    let root = Directory::open(root)?;
    if !root
      .path()
      .canonicalize()
      .map_err(|_| LocalGitError::Io)?
      .starts_with(scratch)
    {
      return Err(LocalError::Unsupported.into());
    }
    // Git ignores its metadata directory even when a hook stages all files in this private copy.
    let metadata = root.child(OsStr::new(".git"), true)?.ok_or(LocalGitError::Io)?;
    let file = windows::pin_store_entry(&metadata, OsStr::new(".orchester-local-owner"), Pin::Anchor)?;
    let ownership = Arc::new(Self {
      directories: vec![root, metadata],
      files: vec![file],
    });
    ownership.ensure_current()?;
    Ok(ownership)
  }
  pub fn ensure_current(&self) -> Result<(), LocalGitError> {
    for directory in &self.directories {
      directory.ensure_current()?;
    }
    for file in &self.files {
      windows::validate_regular(file)?;
    }
    Ok(())
  }
}
