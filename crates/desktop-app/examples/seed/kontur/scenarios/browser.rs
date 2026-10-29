//! `browser`, `browser-not-found` and `browser-chosen`: Settings › Browser (`09 · Settings · Browser` `j3Lre`, `09b ·
//! … not found` `NZECi` in `Browser · redesign`), deterministic and fixture-only. The QA app looks for installed
//! browsers only in [`FOLDER`] beside the database when the seed made it (`examples/qa_app`), never in the computer's
//! own places; the programs there are empty stand-ins, which no QA copy starts. `browser`: Google Chrome is installed
//! there. `browser-not-found`: nothing is. `browser-chosen`: Chrome is, and Chromium in `programs/` beside it is the
//! program chosen in Settings.

use std::{
  fs,
  path::{Path, PathBuf},
};

use orchester_store::AppDb;

use crate::kontur::{Kontur, SeedError, io_error as io};

/// The folder beside the application database the QA app looks for installed browsers in.
pub const FOLDER: &str = "browsers";

/// Where an install of Google Chrome, and of Chromium, below a folder puts its program.
#[cfg(target_os = "windows")]
const CHROME: &str = r"Google\Chrome\Application\chrome.exe";
#[cfg(not(target_os = "windows"))]
const CHROME: &str = "Google Chrome.app/Contents/MacOS/Google Chrome";
#[cfg(target_os = "windows")]
const CHROMIUM: &str = r"Chromium\Application\chrome.exe";
#[cfg(not(target_os = "windows"))]
const CHROMIUM: &str = "Chromium.app/Contents/MacOS/Chromium";

pub fn seed(kontur: &mut Kontur) -> Result<(), SeedError> {
  kontur.catalogue()?;
  install(&beside(&kontur.app_database, FOLDER), CHROME).map(drop)
}

pub fn not_found(kontur: &mut Kontur) -> Result<(), SeedError> {
  kontur.catalogue()?;
  let folder = beside(&kontur.app_database, FOLDER);
  fs::create_dir_all(&folder).map_err(io(&folder))
}

pub fn chosen(kontur: &mut Kontur) -> Result<(), SeedError> {
  seed(kontur)?;
  let chromium = install(&beside(&kontur.app_database, "programs"), CHROMIUM)?;
  AppDb::open(&kontur.app_database)?.set_browser_program(Some(&chromium))?;
  Ok(())
}

fn beside(database: &Path, name: &str) -> PathBuf {
  database.parent().unwrap_or(Path::new(".")).join(name)
}

/// An empty stand-in for a browser's `program` below `folder`.
fn install(folder: &Path, program: &str) -> Result<PathBuf, SeedError> {
  let path = folder.join(program);
  let parent = path.parent().unwrap_or(folder);
  fs::create_dir_all(parent).map_err(io(parent))?;
  fs::write(&path, "").map_err(io(&path))?;
  Ok(path)
}
