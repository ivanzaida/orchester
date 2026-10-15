//! A new file only its owner can read: the team server's TLS key and database snapshots, Orchester Cloud's root key.

use std::{
  fs::{File, OpenOptions},
  io,
  path::Path,
};

/// Creates `path`, which must not exist yet. Unix: mode 0600. Windows: the file takes its folder's access list, which
/// for a folder in the user's profile or the server's data directory admits only that user and administrators; keep
/// such files in such a folder (`docs/server-hosting.md`, `docs/cloud-hosting.md`).
pub fn create(path: &Path) -> io::Result<File> {
  let mut options = OpenOptions::new();
  options.write(true).create_new(true);
  #[cfg(unix)]
  {
    use std::os::unix::fs::OpenOptionsExt;
    options.mode(0o600);
  }
  options.open(path)
}
