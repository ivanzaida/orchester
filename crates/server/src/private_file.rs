//! New files only the server's user can read: database snapshots and the TLS key.

use std::{
  fs::{File, OpenOptions},
  io,
  path::Path,
};

/// Creates `path`, which must not exist yet. Unix: mode 0600. Windows: the file takes its folder's access list, which
/// for the data directory admits the server's user and administrators.
pub(crate) fn create(path: &Path) -> io::Result<File> {
  let mut options = OpenOptions::new();
  options.write(true).create_new(true);
  #[cfg(unix)]
  {
    use std::os::unix::fs::OpenOptionsExt;
    options.mode(0o600);
  }
  options.open(path)
}
