//! Persistent Git roots need nonempty anchors: NTFS refuses reparse conversion of a nonempty directory.
//! Holding a child without sharing deletion keeps every ancestor nonempty while Git uses pathname APIs.
use std::{ffi::OsStr, fs::File, path::Path, sync::Arc};

use super::{Directory, LocalError, LocalGitError, windows};

pub struct ExecutionOwnership {
  directories: Vec<Arc<Directory>>,
  files: Vec<File>,
  metadata: Option<super::metadata::MetadataOwnership>,
}
impl ExecutionOwnership {
  pub fn acquire(root: &Arc<Directory>, scratch: &Arc<Directory>) -> Result<Arc<Self>, LocalGitError> {
    let mut directories = vec![root.clone(), scratch.clone()];
    let mut files = vec![windows::pin_store_entry(
      scratch,
      OsStr::new(".orchester-local-owner"),
      true,
    )?];
    match root.child(OsStr::new(".git"), false) {
      Ok(Some(git)) => {
        files.push(windows::pin_store_entry(
          &git,
          OsStr::new(".orchester-local-owner"),
          true,
        )?);
        directories.push(git);
      }
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
    let metadata = super::metadata::MetadataOwnership::acquire(root)?;
    let ownership = Arc::new(Self {
      directories,
      files,
      metadata: Some(metadata),
    });
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
    let file = windows::pin_store_entry(&metadata, OsStr::new(".orchester-local-owner"), true)?;
    let ownership = Arc::new(Self {
      directories: vec![root, metadata],
      files: vec![file],
      metadata: None,
    });
    ownership.ensure_current()?;
    Ok(ownership)
  }
  pub fn ensure_current(&self) -> Result<(), LocalGitError> {
    if let Some(metadata) = &self.metadata {
      metadata.ensure_current()?;
    }
    for directory in &self.directories {
      directory.ensure_current()?;
    }
    for file in &self.files {
      windows::validate_regular(file)?;
    }
    Ok(())
  }
}
