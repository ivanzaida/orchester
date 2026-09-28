//! macOS (and other Unix systems): the folder is `0700` and each file `0600`, so only the user
//! reads them; the temporary folder (`$TMPDIR`) is the user's own on macOS.

use std::{
  fs::{DirBuilder, File, OpenOptions},
  io,
  os::unix::fs::{DirBuilderExt, OpenOptionsExt},
  path::Path,
};

pub(super) fn create_folder(folder: &Path) -> io::Result<()> {
  DirBuilder::new().recursive(true).mode(0o700).create(folder)
}

pub(super) fn create_file(path: &Path) -> io::Result<File> {
  OpenOptions::new().write(true).create_new(true).mode(0o600).open(path)
}
