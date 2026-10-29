//! Finding the browser on macOS: the app bundles of Google Chrome, Microsoft Edge, Brave and Chromium, in that order of
//! preference, each in `/Applications`, then in the person's `~/Applications`. A program the person chose is one of
//! theirs, by its name, wherever it is; an app bundle chosen stands for the program inside it.

use std::{
  env,
  ffi::OsStr,
  path::{Path, PathBuf},
};

use super::{BrowserProgram, BrowserUnavailable, first_installed, listed};

/// Each browser's name and its program inside its app bundle, in order of preference.
const BROWSERS: [(&str, &str); 4] = [
  ("Google Chrome", "Google Chrome.app/Contents/MacOS/Google Chrome"),
  ("Microsoft Edge", "Microsoft Edge.app/Contents/MacOS/Microsoft Edge"),
  ("Brave", "Brave Browser.app/Contents/MacOS/Brave Browser"),
  ("Chromium", "Chromium.app/Contents/MacOS/Chromium"),
];

pub(super) fn find() -> Result<BrowserProgram, BrowserUnavailable> {
  let home = env::var_os("HOME").map(PathBuf::from);
  first_installed(
    candidates(&folders(home.as_deref())),
    "/Applications and ~/Applications",
  )
}

pub(super) fn find_in(folders: &[PathBuf]) -> Result<BrowserProgram, BrowserUnavailable> {
  first_installed(candidates(folders), &listed(folders))
}

/// The program inside `path` when it is an app bundle (`Contents/MacOS/` and the bundle's name, as each browser's
/// is), else `path` itself.
pub(super) fn program_in(path: &Path) -> PathBuf {
  match path.file_stem() {
    Some(stem) if path.extension() == Some(OsStr::new("app")) && path.is_dir() => {
      path.join("Contents").join("MacOS").join(stem)
    }
    _ => path.to_path_buf(),
  }
}

/// The browser whose program `program` is, by its file name.
pub(super) fn named(program: &Path) -> Option<&'static str> {
  let file = program.file_name()?;
  BROWSERS
    .iter()
    .find(|(_, inside)| Path::new(inside).file_name() == Some(file))
    .map(|(name, _)| *name)
}

/// The folders searched: `/Applications`, then `~/Applications` of `home`, when it is known.
pub fn folders(home: Option<&Path>) -> Vec<PathBuf> {
  let mut folders = vec![PathBuf::from("/Applications")];
  folders.extend(home.map(|home| home.join("Applications")));
  folders
}

/// Each browser's program in each of `folders`, a preferred browser before the next wherever each is.
pub fn candidates(folders: &[PathBuf]) -> Vec<(&'static str, PathBuf)> {
  BROWSERS
    .iter()
    .flat_map(|&(name, program)| folders.iter().map(move |folder| (name, folder.join(program))))
    .collect()
}
