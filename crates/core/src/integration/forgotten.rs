//! Worktree folders of a run that git no longer lists: `git worktree remove` that failed part way
//! (on Windows a file held open) can forget a worktree and leave its folder. Such a folder is still
//! there for the person to remove, with the files in it listed like uncommitted changes, so it goes
//! only as they were seen. It is deleted as a folder, and only ever
//! `.orchester/worktrees/<run or step execution id>`, a path made here, never one read elsewhere.

use std::{
  fs,
  path::{Path, PathBuf},
};

use super::{GitError, LeftWorktree, WORKTREES_DIR, WorktreeOwner};
use crate::run::Run;

/// The folders of `run`'s worktrees under `repo` that are not among `listed`, what git lists, with
/// the files in them.
pub(super) fn forgotten(repo: &Path, run: &Run, listed: &[LeftWorktree]) -> Result<Vec<LeftWorktree>, GitError> {
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
      let display = path.to_string_lossy().replace('\\', "/");
      let mut files = Vec::new();
      list_files(&path, &path, &mut files).map_err(|error| GitError::Folder {
        path: display.clone(),
        detail: error.to_string(),
      })?;
      files.sort();
      Ok(LeftWorktree {
        path: display,
        owner,
        branch: None,
        uncommitted: files,
        missing: false,
        forgotten: true,
      })
    })
    .collect()
}

/// The files under `dir`, relative to `root`, with `/` separators.
fn list_files(root: &Path, dir: &Path, files: &mut Vec<String>) -> std::io::Result<()> {
  for entry in fs::read_dir(dir)? {
    let entry = entry?;
    let path: PathBuf = entry.path();
    if entry.file_type()?.is_dir() {
      list_files(root, &path, files)?;
    } else {
      let relative = path.strip_prefix(root).unwrap_or(&path);
      files.push(relative.to_string_lossy().replace('\\', "/"));
    }
  }
  Ok(())
}

/// Deletes the folder of `worktree`, which git forgot; why not, when it stays.
pub(super) fn delete(worktree: &LeftWorktree) -> Result<(), String> {
  fs::remove_dir_all(&worktree.path).map_err(|error| error.to_string())
}
