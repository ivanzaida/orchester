//! Worktree folders of a run that git no longer lists: `git worktree remove` that failed part way
//! (on Windows a file held open) can forget a worktree and leave its folder. Such a folder is still
//! there for the person to remove; it is deleted as a folder, and only ever
//! `.orchester/worktrees/<run or step execution id>`, a path made here, never one read elsewhere.

use std::{fs, path::Path};

use super::{LeftWorktree, WORKTREES_DIR, WorktreeOwner};
use crate::run::Run;

/// The folders of `run`'s worktrees under `repo` that are not among `listed`, what git lists.
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
    .map(|(path, owner)| LeftWorktree {
      path: path.to_string_lossy().replace('\\', "/"),
      owner,
      branch: None,
      uncommitted: Vec::new(),
      missing: false,
      forgotten: true,
    })
    .collect()
}

/// Deletes the folder of `worktree`, which git forgot; why not, when it stays.
pub(super) fn delete(worktree: &LeftWorktree) -> Result<(), String> {
  fs::remove_dir_all(&worktree.path).map_err(|error| error.to_string())
}
