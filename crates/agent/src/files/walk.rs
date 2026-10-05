//! The folder walk of `list_files` and `search` without ripgrep (`docs/architecture/native-agent.md` §4.2,
//! §4.3). Like ripgrep by default it skips hidden entries (`.git` among them) and does not follow
//! symbolic links or junctions, so it never leaves the folder it was given; unlike ripgrep it does not
//! read `.gitignore`, which the results say. It stops at a number of files, a folder depth and a
//! deadline, so that a huge tree cannot hold the call.

use std::{
  fs,
  path::{Path, PathBuf},
  time::{Duration, Instant},
};

/// The most files one walk collects.
pub(super) const MAX_WALKED: usize = 100_000;
/// Folders deeper than this below the root are not entered.
const MAX_DEPTH: usize = 64;
/// How long a call without ripgrep may take, as long as a ripgrep run.
pub(super) const WALK_TIMEOUT: Duration = Duration::from_secs(60);

/// The files under `root`, or `root` itself when it is a file, sorted; whether the walk stopped early
/// (at [`MAX_WALKED`] files or at `deadline`).
pub(super) fn files(root: &Path, deadline: Instant) -> (Vec<PathBuf>, bool) {
  if root.is_file() {
    return (vec![root.to_path_buf()], false);
  }
  let mut found = Vec::new();
  let mut folders = vec![(root.to_path_buf(), 0)];
  let mut stopped = false;
  while let Some((folder, depth)) = folders.pop() {
    if Instant::now() > deadline {
      stopped = true;
      break;
    }
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
      if kind.is_dir() && depth < MAX_DEPTH {
        folders.push((entry.path(), depth + 1));
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
  (found, stopped)
}
