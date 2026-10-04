//! Worktree folders of a run that git no longer lists: `git worktree remove` that failed part way
//! (on Windows a file held open) can forget a worktree and leave its folder. Such a folder is still
//! there for the person to remove, with the files in it listed like uncommitted changes, so it goes
//! only as they were seen. It is deleted as a folder, and only ever
//! `.orchester/worktrees/<run or step execution id>`, a path made here, never one read elsewhere.
//!
//! What runs left is read every time a task page shows them, and such a folder often holds a whole
//! build's output, so only its first [`FOLDER_ENTRIES_READ`] entries are read, in name order; past
//! them it is known to hold more ([`FolderListing::Cut`]). An entry that cannot be read ends the
//! listing of that folder with why ([`FolderListing::Unreadable`]); the others are read as usual.

use std::{
  fs, io,
  path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use super::{LeftWorktree, WORKTREES_DIR, WorktreeOwner};
use crate::run::Run;

/// Entries of a forgotten folder read at most, folders among them.
pub const FOLDER_ENTRIES_READ: usize = 200;

/// How much of a worktree's files [`LeftWorktree::uncommitted`] lists. Only a forgotten folder's
/// listing can be short of all of them.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FolderListing {
  #[default]
  Whole,
  /// The folder holds more than the files listed: reading stopped after [`FOLDER_ENTRIES_READ`]
  /// entries.
  Cut,
  /// The entry at `path` could not be read, `detail` says why; the files read before it are listed.
  Unreadable { path: String, detail: String },
}

/// The folders of `run`'s worktrees under `repo` that are not among `listed`, what git lists, with
/// the files in them.
pub(super) fn forgotten(repo: &Path, run: &Run, listed: &[LeftWorktree]) -> Vec<LeftWorktree> {
  let executions = run
    .attempts()
    .iter()
    .flat_map(|attempt| attempt.executions())
    .map(|execution| (execution.id().to_string(), WorktreeOwner::Step(execution.id())));
  std::iter::once((run.id().to_string(), WorktreeOwner::Run))
    .chain(executions)
    .filter(|(name, _)| {
      !listed.iter().any(|worktree| {
        Path::new(&worktree.path)
          .file_name()
          .is_some_and(|listed| listed == name.as_str())
      })
    })
    .map(|(name, owner)| (repo.join(WORKTREES_DIR).join(name), owner))
    .filter(|(path, _)| path.is_dir())
    .map(|(path, owner)| {
      let (uncommitted, listing) = list(&path);
      LeftWorktree {
        path: slashed(&path),
        owner,
        branch: None,
        uncommitted,
        missing: false,
        forgotten: true,
        listing,
      }
    })
    .collect()
}

/// The files under `root`, relative to it with `/` separators and sorted, and how far they were
/// read.
fn list(root: &Path) -> (Vec<String>, FolderListing) {
  let mut walk = Walk {
    root,
    files: Vec::new(),
    entries_left: FOLDER_ENTRIES_READ,
  };
  let listing = match walk.dir(root) {
    Ok(true) => FolderListing::Whole,
    Ok(false) => FolderListing::Cut,
    Err((path, error)) => FolderListing::Unreadable {
      path: slashed(&path),
      detail: error.to_string(),
    },
  };
  walk.files.sort();
  (walk.files, listing)
}

/// A listing of a folder's files that reads its entries in name order, so two listings of a folder
/// that did not change stop at the same entry.
struct Walk<'a> {
  root: &'a Path,
  files: Vec<String>,
  entries_left: usize,
}

impl Walk<'_> {
  /// Lists the files under `dir`: whether every entry was read, or the entry that could not be.
  fn dir(&mut self, dir: &Path) -> Result<bool, (PathBuf, io::Error)> {
    let mut entries = fs::read_dir(dir)
      .and_then(|entries| entries.collect::<io::Result<Vec<_>>>())
      .map_err(|error| (dir.to_path_buf(), error))?;
    entries.sort_by_key(fs::DirEntry::file_name);
    for entry in entries {
      if self.entries_left == 0 {
        return Ok(false);
      }
      self.entries_left -= 1;
      let path = entry.path();
      let kind = entry.file_type().map_err(|error| (path.clone(), error))?;
      if kind.is_dir() {
        if !self.dir(&path)? {
          return Ok(false);
        }
      } else {
        let relative = path.strip_prefix(self.root).unwrap_or(&path);
        self.files.push(slashed(relative));
      }
    }
    Ok(true)
  }
}

fn slashed(path: &Path) -> String {
  path.to_string_lossy().replace('\\', "/")
}

/// Deletes the folder of `worktree`, which git forgot; why not, when it stays.
pub(super) fn delete(worktree: &LeftWorktree) -> Result<(), String> {
  fs::remove_dir_all(&worktree.path).map_err(|error| error.to_string())
}
