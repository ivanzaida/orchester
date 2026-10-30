//! A worktree removal that could not finish: something holds a file in its folder, on Windows a process of the run (a
//! QA copy, a build) with a file open. [`retry_held`] tries such worktrees again once the caller ended the run's own
//! processes there; one that still stays is a worktree not removed ([`NotRemoved`]), recorded with its path and why
//! ([`finish_removal`]), never a removed one.

use std::path::Path;

use super::{
  Cleanup, KeepReason,
  cleanup::remove_listed,
  forgotten,
  git::Git,
};
use crate::{command::CommandRunner, id::RunId, store::WorktreeStore, time::Timestamp};

/// Ends the removal of `run`'s worktrees `cleanup` did in the repository at `repo`: tries again those held
/// ([`retry_held`]) once `stop` ended what of the run runs in each folder, then records in `store` at `at` the folder
/// that still stays and why, or that none does.
pub fn finish_removal<S: WorktreeStore>(
  store: &mut S,
  (runner, git, repo): (&impl CommandRunner, &Path, &Path),
  (run, at): (RunId, Timestamp),
  cleanup: &mut Cleanup,
  stop: impl FnMut(&Path),
) -> Result<(), S::Error> {
  retry_held(runner, (git, repo), cleanup, stop);
  match cleanup.not_removed() {
    Some(left) => store.save_worktree_not_removed(run, &left, at),
    None => store.clear_worktree_not_removed(run),
  }
}

/// A worktree of a run that removing it left on disk: its folder and why it stays.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NotRemoved {
  /// Its folder, as git printed it or Orchester made it.
  pub path: String,
  /// What git or the file system said; `None` for a folder found there without a record of why.
  pub reason: Option<String>,
}

impl Cleanup {
  /// The first worktree removing kept because git or the file system refused: it stays on disk, with why.
  pub fn not_removed(&self) -> Option<NotRemoved> {
    self.kept_worktrees.iter().find_map(|(worktree, reason)| match reason {
      KeepReason::Failed(detail) => Some(NotRemoved {
        path: worktree.path.clone(),
        reason: Some(detail.clone()),
      }),
      _ => None,
    })
  }
}

/// Removes again the worktrees `cleanup` kept because git or the file system refused, in the repository at `repo` with
/// the `git` program, once `stop` ended what of the run still runs in each folder. Those removed now move to
/// `cleanup.removed`; the others stay kept with what was said this time.
pub fn retry_held(
  runner: &impl CommandRunner,
  (git, repo): (&Path, &Path),
  cleanup: &mut Cleanup,
  mut stop: impl FnMut(&Path),
) {
  let git = Git::new(runner, git, repo);
  for (mut worktree, reason) in std::mem::take(&mut cleanup.kept_worktrees) {
    let folder = Path::new(&worktree.path);
    if !matches!(reason, KeepReason::Failed(_)) || !folder.exists() {
      cleanup.kept_worktrees.push((worktree, reason));
      continue;
    }
    stop(folder);
    let outcome = if worktree.forgotten {
      forgotten::delete(&worktree)
    } else {
      // Kept for a refusal, not for changes: whatever it would lose was agreed to the first time.
      let removed = remove_listed(&git, &worktree.path, worktree.loses_files());
      worktree.forgotten = removed.as_ref().is_err_and(|refusal| refusal.forgotten);
      removed.map_err(|refusal| refusal.detail)
    };
    match outcome {
      Ok(()) => cleanup.removed.push(worktree),
      Err(detail) => cleanup.kept_worktrees.push((worktree, KeepReason::Failed(detail))),
    }
  }
}
