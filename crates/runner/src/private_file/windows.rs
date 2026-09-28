//! Windows: the temporary folder is in the user's profile (`%LOCALAPPDATA%\Temp`), whose access
//! list gives the user, the system and administrators access and nobody else; the folder and the
//! file inherit it. No access list of their own is set: that needs the security API through
//! `unsafe` FFI, and the inherited one already keeps other users out. A `TEMP` set to a shared
//! folder would weaken this.

use std::{
  fs::{self, File, OpenOptions},
  io,
  path::Path,
};

pub(super) fn create_folder(folder: &Path) -> io::Result<()> {
  fs::create_dir_all(folder)
}

pub(super) fn create_file(path: &Path) -> io::Result<File> {
  OpenOptions::new().write(true).create_new(true).open(path)
}
