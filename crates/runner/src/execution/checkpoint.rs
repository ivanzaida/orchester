//! Checkpoint commits: everything a step changed in the run's worktree, committed on the run's
//! branch with the repository's git identity.

use std::path::Path;

use orchester_core::revision::Revision;

use crate::{git::Git, worktree::WorktreeError};

/// Commits every change in `worktree` with `message`; `None` when nothing changed.
pub(super) fn commit(worktree: &Path, message: &str) -> Result<Option<Revision>, WorktreeError> {
  let git = Git::new(worktree);
  if git
    .stdout(["status", "--porcelain", "--untracked-files=all"])?
    .is_empty()
  {
    return Ok(None);
  }
  git.stdout(["add", "--all"])?;
  git.stdout(["commit", "--quiet", "-m", message])?;
  let head = git.stdout(["rev-parse", "HEAD"])?;
  Revision::new(&head)
    .map(Some)
    .map_err(|_| WorktreeError::UnexpectedRevision(head))
}
