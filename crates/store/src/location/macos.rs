//! The data location of a macOS app bundle. `Orchester.app/Contents/MacOS/` is part of the
//! signed bundle and not writable for the app (it may sit in `/Applications`, and an update
//! replaces the whole bundle), so a bundled build keeps its data in the user's Library, where
//! Apple's file system guidelines put it:
//!
//! - the application database in `~/Library/Application Support/Orchester/`;
//! - the application log in `~/Library/Logs/Orchester/`, the folder Console.app lists under *Log Reports*, rather than
//!   in Application Support, so the log is found where macOS users and support look for logs and is not mixed with data
//!   a backup must keep.
//!
//! A build outside a bundle (`cargo run`, a development build in `target/`) keeps both beside
//! its executable like on Windows, so development builds never share the installed app's
//! database: a dev build of an unmerged branch would otherwise migrate it.

use std::{ffi::OsStr, io, path::Path};

use super::{APP_DATABASE, AppDataLocation};

/// The folder named after the app in `Application Support` and in `Logs`.
const APP_FOLDER: &str = "Orchester";

/// Where a build at `executable` keeps its data, given the user's home folder (`HOME`).
/// Fails only for a bundled build without a home folder.
pub fn location_of(executable: &Path, home: Option<&Path>) -> io::Result<AppDataLocation> {
  if !in_app_bundle(executable) {
    return Ok(AppDataLocation::beside(executable));
  }
  let home = home.filter(|home| home.is_absolute()).ok_or_else(|| {
    io::Error::new(
      io::ErrorKind::NotFound,
      "HOME is not set to an absolute path, so the app bundle has no Library folder for its data",
    )
  })?;
  Ok(library_location(home))
}

/// `~/Library/Application Support/Orchester/orchester.db` and `~/Library/Logs/Orchester/`.
pub fn library_location(home: &Path) -> AppDataLocation {
  let library = home.join("Library");
  AppDataLocation {
    database: library.join("Application Support").join(APP_FOLDER).join(APP_DATABASE),
    log_dir: library.join("Logs").join(APP_FOLDER),
  }
}

/// Whether `executable` is the program of an app bundle: `<name>.app/Contents/MacOS/<program>`.
pub fn in_app_bundle(executable: &Path) -> bool {
  let mut folders = executable.ancestors().skip(1);
  let (Some(programs), Some(contents), Some(bundle)) = (folders.next(), folders.next(), folders.next()) else {
    return false;
  };
  programs.file_name() == Some(OsStr::new("MacOS"))
    && contents.file_name() == Some(OsStr::new("Contents"))
    && bundle.extension() == Some(OsStr::new("app"))
}
