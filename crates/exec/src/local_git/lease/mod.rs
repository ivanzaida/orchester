//! An OS-held project lease plus stable ownership of its metadata parent chain.

use std::{fs::File, path::Path, sync::Arc};

use orchester_core::integration::local::LocalPath;

use super::{
  LocalError, LocalGitError,
  owned::{Directory, OwnedFile},
};

pub struct IntegrationLease {
  _file: File,
  _directory: Arc<Directory>,
}

impl IntegrationLease {
  pub fn acquire(root: &Path) -> Result<Self, LocalGitError> {
    let directory = super::files::hold_directory(root, &[".orchester"], true)?.ok_or(LocalGitError::Io)?;
    super::files::owned_store(root)?;
    let path = LocalPath::new("local-integration.lock")?;
    let opened = match OwnedFile::open(directory.path(), &path, true) {
      Ok(Some(file)) => file,
      Ok(None) => OwnedFile::create(directory.path(), &path).map_err(|_| LocalError::ForeignOperation)?,
      Err(_) => return Err(LocalError::ForeignOperation.into()),
    };
    let file = opened.into_file();
    #[cfg(unix)]
    rustix::fs::flock(&file, rustix::fs::FlockOperation::NonBlockingLockExclusive)
      .map_err(|_| LocalError::ForeignOperation)?;
    Ok(Self {
      _file: file,
      _directory: directory,
    })
  }
}
