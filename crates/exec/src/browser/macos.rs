//! Finding the browser on macOS: the app bundles of Google Chrome, Microsoft Edge, Brave and Chromium, in that order of
//! preference, each in `/Applications`, then in the person's `~/Applications`.

use std::{
  env,
  path::{Path, PathBuf},
};

use super::{BrowserProgram, BrowserUnavailable, first_installed};

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
