//! A new file only its owner can read, for the root key.

use std::{fs::File, io, path::Path};

/// Unix: created with mode 0600.
#[cfg(unix)]
pub(crate) fn create(path: &Path) -> io::Result<File> {
  use std::os::unix::fs::OpenOptionsExt;
  std::fs::OpenOptions::new()
    .write(true)
    .create_new(true)
    .mode(0o600)
    .open(path)
}

/// Windows: a new file takes its folder's access list, which for a folder in the user's profile admits only the user
/// and administrators; keep the root key in such a folder (`docs/cloud-hosting.md`).
#[cfg(not(unix))]
pub(crate) fn create(path: &Path) -> io::Result<File> {
  std::fs::OpenOptions::new().write(true).create_new(true).open(path)
}
