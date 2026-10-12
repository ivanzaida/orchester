//! A non-blocking OS-held lease shared by integration clients. A crash releases it without deleting recovery data.

use std::{fs::File, path::Path};

use super::LocalGitError;

#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

pub struct IntegrationLease {
  _file: File,
}

impl IntegrationLease {
  pub fn acquire(root: &Path) -> Result<Self, LocalGitError> {
    super::files::owned_store(root)?;
    let path = root.join(".orchester/local-integration.lock");
    if std::fs::symlink_metadata(&path).is_ok_and(|metadata| metadata.file_type().is_symlink()) {
      return Err(super::LocalError::Unsupported.into());
    }
    #[cfg(windows)]
    let file = windows::acquire(&path)?;
    #[cfg(unix)]
    let file = unix::acquire(&path)?;
    #[cfg(not(any(windows, unix)))]
    return Err(super::LocalError::Unsupported.into());
    #[cfg(any(windows, unix))]
    Ok(Self { _file: file })
  }
}
