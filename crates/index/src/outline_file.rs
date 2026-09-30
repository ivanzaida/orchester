//! Where a source file's outline goes: `outline/<path>.md` inside the index, for a path that every
//! platform keeps as that one file. Tracked paths come from git, which accepts names Windows cannot
//! hold (`a:b.rs`, `q?.ts`, `CON.rs`, a trailing dot), names that climb out of a folder on Windows
//! (`x\..\y.rs`), and names that differ only in case, which share one file on Windows and macOS. A
//! path like that keeps its row in `tree.txt` and its symbols, but has no outline, on every platform
//! alike, so the index does not depend on where it was built.

use std::{
  collections::{HashMap, HashSet},
  path::{Path, PathBuf},
};

const OUTLINES: &str = "outline";
/// Characters Windows does not allow in a file name, besides control characters.
const RESERVED: [char; 9] = ['\\', '/', ':', '*', '?', '"', '<', '>', '|'];
/// Windows device names, which name no file whatever their extension.
const DEVICES: [&str; 4] = ["con", "prn", "aux", "nul"];
const NUMBERED_DEVICES: [&str; 2] = ["com", "lpt"];

/// `outline/<path>.md` of the index in `dir`, for a path of [`outlined`].
pub(crate) fn outline_path(dir: &Path, path: &str) -> PathBuf {
  let mut file = path
    .split('/')
    .fold(dir.join(OUTLINES), |folder, name| folder.join(name));
  file.as_mut_os_string().push(".md");
  file
}

/// The outline file of `path` as the index's digests name it: `outline/<path>.md`.
pub(crate) fn outline_name(path: &str) -> String {
  format!("{OUTLINES}/{path}.md")
}

/// Which of `paths` get an outline file: the portable ones that share it with no other path when
/// case is ignored.
pub(crate) fn outlined<'a>(paths: impl Iterator<Item = &'a str>) -> HashSet<&'a str> {
  let mut by_folded: HashMap<String, Vec<&str>> = HashMap::new();
  for path in paths.filter(|path| portable(path)) {
    by_folded.entry(path.to_lowercase()).or_default().push(path);
  }
  by_folded
    .into_values()
    .filter(|paths| paths.len() == 1)
    .flatten()
    .collect()
}

/// Whether every `/`-separated part of `path` is a plain file name on Windows, macOS and Linux.
fn portable(path: &str) -> bool {
  path.split('/').all(|name| {
    let plain = !name.is_empty()
      && name != "."
      && name != ".."
      && !name.ends_with(['.', ' '])
      && !name
        .chars()
        .any(|character| character.is_control() || RESERVED.contains(&character));
    plain && !device(name)
  })
}

/// Whether `name` is a Windows device name, with or without an extension.
fn device(name: &str) -> bool {
  let stem = name.split('.').next().unwrap_or(name).trim_end().to_ascii_lowercase();
  DEVICES.contains(&stem.as_str())
    || NUMBERED_DEVICES.iter().any(|prefix| {
      stem
        .strip_prefix(prefix)
        .is_some_and(|number| number.len() == 1 && number.chars().all(|digit| digit.is_ascii_digit()))
    })
}
