//! Checkpoint commits and merges: everything a step changed in its worktree, committed on the
//! worktree's branch (or the commits its agent made there itself), and a step branch merged back
//! into the run's branch — both with the repository's git identity.

use std::path::Path;

use orchester_core::revision::Revision;

use crate::{git::Git, worktree::WorktreeError};

/// Commits every change in `worktree` with `message`; `None` when nothing changed.
fn commit(worktree: &Path, message: &str) -> Result<Option<Revision>, WorktreeError> {
  let git = Git::new(worktree);
  if git
    .stdout(&["status", "--porcelain", "--untracked-files=all"])?
    .is_empty()
  {
    return Ok(None);
  }
  git.stdout(&["add", "--all"])?;
  git.stdout(&["commit", "--quiet", "-m", message])?;
  head(worktree).map(Some)
}

/// What a step left in its worktree when it ended.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Saved {
  /// Its work is at this commit: what it left uncommitted, committed now, or else the commits its agent made itself.
  At(Revision),
  /// Nothing new.
  Nothing,
  /// Its agent's commits are on `head` (another branch, or a detached HEAD), not on `branch`, the worktree's: they are
  /// not taken as its work.
  Elsewhere { head: String, branch: String },
}

/// Commits every change in `worktree` with `message`; with nothing to commit, takes the commits the step's agent made
/// itself on `branch`, the worktree's, when its HEAD is none of the `known` revisions (where the step started, the
/// run's last checkpoint, the run's start).
pub(super) fn save(
  worktree: &Path,
  message: &str,
  (branch, known): (&str, &[&Revision]),
) -> Result<Saved, WorktreeError> {
  if let Some(revision) = commit(worktree, message)? {
    return Ok(Saved::At(revision));
  }
  let head = head(worktree)?;
  if known.contains(&&head) {
    return Ok(Saved::Nothing);
  }
  let on = Git::new(worktree).attempt(&["symbolic-ref", "--quiet", "--short", "HEAD"])?;
  if on.as_deref() == Ok(branch) {
    return Ok(Saved::At(head));
  }
  Ok(Saved::Elsewhere {
    head: on.unwrap_or_else(|_| "a detached HEAD".to_owned()),
    branch: branch.to_owned(),
  })
}

/// The commit checked out in `worktree`.
pub(super) fn head(worktree: &Path) -> Result<Revision, WorktreeError> {
  let head = Git::new(worktree).stdout(&["rev-parse", "HEAD"])?;
  Revision::new(&head).map_err(|_| WorktreeError::UnexpectedRevision(head))
}

/// How merging a step's branch back went.
pub(super) enum Merge {
  /// The run's branch is at this commit now: the step branch fast-forwarded it, a merge commit
  /// joined them, or it had nothing new.
  Merged(Revision),
  /// Both changed these files; the merge was undone and the run's branch is as it was.
  Conflict(Vec<String>),
}

/// Merges `branch` into the branch checked out in `worktree` (the run's), with `message` when it
/// takes a merge commit.
pub(super) fn merge(worktree: &Path, branch: &str, message: &str) -> Result<Merge, WorktreeError> {
  let git = Git::new(worktree);
  if let Err(refusal) = git.attempt(&["merge", "--no-edit", "-m", message, branch])? {
    // A conflict leaves a merge in progress; a refusal before merging (such as local changes in
    // the way) does not, and is a failure of git rather than of the step.
    if !git.answer(&["rev-parse", "--quiet", "--verify", "MERGE_HEAD"])? {
      return Err(WorktreeError::Git {
        command: format!("merge {branch}"),
        code: None,
        stderr: refusal,
      });
    }
    let files = git.stdout(&["diff", "--name-only", "--diff-filter=U"])?;
    git.stdout(&["merge", "--abort"])?;
    return Ok(Merge::Conflict(files.lines().map(str::to_owned).collect()));
  }
  head(worktree).map(Merge::Merged)
}
