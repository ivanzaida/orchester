//! `browser`, `browser-not-found` and `browser-chosen`: Settings › Browser (`09 · Settings · Browser` `j3Lre`, `09b ·
//! … not found` `NZECi` in `Browser · redesign`), deterministic and fixture-only. The QA app looks for installed
//! browsers only in [`FOLDER`] beside the database when the seed made it (`examples/qa_app`), never in the computer's
//! own places; the programs there are empty stand-ins, which no QA copy starts. `browser`: Google Chrome is installed
//! there, its install telling [`VERSION`]. `browser-not-found`: nothing is. `browser-chosen`: Chrome is, and Chromium
//! in `programs/` beside it is the program chosen in Settings, whose install tells no version.

use std::{
  fs,
  path::{Path, PathBuf},
};

use orchester_store::AppDb;

use crate::kontur::{Kontur, SeedError, io_error as io};

/// The folder beside the application database the QA app looks for installed browsers in.
pub const FOLDER: &str = "browsers";
/// The version the stand-in for Google Chrome tells.
const VERSION: &str = "130.0.6723.92";

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
  let chrome = install(&beside(&kontur.app_database, FOLDER), CHROME)?;
  tell_version(&chrome)
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

/// Makes the install around the stand-in `program` tell [`VERSION`]: a folder named after it beside the program.
#[cfg(target_os = "windows")]
fn tell_version(program: &Path) -> Result<(), SeedError> {
  let folder = program.with_file_name(VERSION);
  fs::create_dir_all(&folder).map_err(io(&folder))
}

/// Makes the install around the stand-in `program` tell [`VERSION`]: the `Info.plist` of its app bundle.
#[cfg(not(target_os = "windows"))]
fn tell_version(program: &Path) -> Result<(), SeedError> {
  let plist = program
    .parent()
    .and_then(Path::parent)
    .unwrap_or(program)
    .join("Info.plist");
  let text = format!("<plist><dict><key>CFBundleShortVersionString</key><string>{VERSION}</string></dict></plist>");
  fs::write(&plist, text).map_err(io(&plist))
}
