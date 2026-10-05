//! The folder walk of `list_files` and `search` without ripgrep (`docs/architecture/native-agent.md` §4.2,
//! §4.3). Like ripgrep by default it skips hidden entries (`.git` among them) and does not follow
//! symbolic links or junctions, so it never leaves the folder it was given; unlike ripgrep it does not
//! read `.gitignore`, which the results say.

use std::{
  fs,
  path::{Path, PathBuf},
};

/// The most files one walk collects.
pub(super) const MAX_WALKED: usize = 100_000;

/// The files under `root`, or `root` itself when it is a file, sorted; whether the walk stopped at
/// [`MAX_WALKED`].
pub(super) fn files(root: &Path) -> (Vec<PathBuf>, bool) {
  if root.is_file() {
    return (vec![root.to_path_buf()], false);
  }
  let mut found = Vec::new();
  let mut folders = vec![root.to_path_buf()];
  while let Some(folder) = folders.pop() {
    // A folder that cannot be listed is left out, as ripgrep leaves it out.
    let Ok(entries) = fs::read_dir(&folder) else {
      continue;
    };
    for entry in entries.flatten() {
      if entry.file_name().to_string_lossy().starts_with('.') {
        continue;
      }
      let Ok(kind) = entry.file_type() else {
        continue;
      };
      if kind.is_dir() {
        folders.push(entry.path());
      } else if kind.is_file() {
        found.push(entry.path());
        if found.len() >= MAX_WALKED {
          found.sort();
          return (found, true);
        }
      }
    }
  }
  found.sort();
  (found, false)
}
