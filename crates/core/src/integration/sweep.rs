//! The sweep of `.orchester/worktrees/`: folders there that no worktree git lists and no run that has not ended owns
//! are what removals that failed part way left (git forgot the worktree, its build output stayed), often hundreds of
//! gigabytes. They go as folders. A folder git lists is never touched, and neither is one whose name no run that
//! ended has: a run that has not ended, or a folder Orchester did not make.

use std::{
  fs, io,
  path::{Path, PathBuf},
};

use thiserror::Error;

use super::{GitError, WORKTREES_DIR, forgotten, git::Git, parse_worktree_list};
use crate::command::CommandRunner;

/// A folder the sweep took on, and whether it went: why not, when it stays.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Swept {
  pub path: PathBuf,
  pub outcome: Result<(), String>,
}

/// Deletes the folders under `repo`'s [`WORKTREES_DIR`] that git, the `git` program, does not list as worktrees and
/// that `ended` says a run that ended owns, by the folder's name: the run's id or one of its step executions'. Fails
/// only when the folder or git's worktrees cannot be read, and then deletes nothing.
pub fn sweep(
  runner: &impl CommandRunner,
  (git, repo): (&Path, &Path),
  ended: impl Fn(&str) -> bool,
) -> Result<Vec<Swept>, SweepError> {
  let folder = repo.join(WORKTREES_DIR);
  let entries = match fs::read_dir(&folder) {
    Ok(entries) => entries.collect::<io::Result<Vec<_>>>()?,
    Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
    Err(error) => return Err(error.into()),
  };
  let listed = parse_worktree_list(&Git::new(runner, git, repo).stdout(&["worktree", "list", "--porcelain", "-z"])?);
  let mut swept = Vec::new();
  for entry in entries {
    // A link is never followed out of the folder, and an entry that cannot be told is left as it is.
    if !entry.file_type().is_ok_and(|kind| kind.is_dir()) {
      continue;
    }
    let name = entry.file_name();
    let Some(name) = name.to_str() else {
      continue;
    };
    let owned_by_git = listed.iter().any(|worktree| {
      Path::new(&worktree.path)
        .file_name()
        .is_some_and(|listed| listed == name)
    });
    if owned_by_git || !ended(name) {
      continue;
    }
    let path = entry.path();
    let outcome = forgotten::delete_folder(&path);
    swept.push(Swept { path, outcome });
  }
  Ok(swept)
}

/// Why the sweep deleted nothing.
#[derive(Debug, Error)]
pub enum SweepError {
  #[error("cannot read the worktrees folder: {0}")]
  Folder(#[from] io::Error),
  #[error(transparent)]
  Git(#[from] GitError),
}
