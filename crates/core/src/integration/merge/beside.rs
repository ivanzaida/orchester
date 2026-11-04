//! Merging a run into its base branch beside the person's checkout (ORC-81): for a project whose base branch is set,
//! while the checkout has another branch checked out ([`crate::integration::base::Landing`]). The checkout is neither
//! switched nor touched. The run's branch fast-forwards the base branch, or the merge commit is made in the temporary
//! worktree as for the checkout ([`super::temporary`]); the base branch's ref then moves to it only while it is still
//! at the commit the merge began from (`git update-ref` with its old value), so a base that moved meanwhile merges
//! nothing. A base branch checked out in a worktree, the checkout's own included, is refused: moving it there would
//! leave that worktree's files behind its branch.

use super::{Delivering, MergeError, MergeKind, Merged, Source, revision, temporary};
use crate::{
  command::CommandRunner,
  integration::{
    GitError,
    git::{Git, message},
    parse_worktree_list,
  },
  run::BranchName,
};

/// Merges `head`, the tip of `source`'s branch, into its base branch beside the checkout `git` runs in.
pub(super) fn merge<R: CommandRunner>(
  git: &Git<'_, R>,
  source: &Source<'_>,
  head: &str,
  delivering: Option<Delivering<'_>>,
) -> Result<Merged, MergeError> {
  let (into, title) = (source.into, source.title.as_str());
  not_checked_out(git, into)?;
  let base = git
    .commit(&format!("refs/heads/{into}"))?
    .ok_or_else(|| MergeError::BaseMissing(into.clone()))?;
  if let Some(delivering) = delivering
    && base != delivering.base.as_str()
  {
    return Err(MergeError::BaseMoved { branch: into.clone() });
  }
  if git.is_ancestor(head, &base)? {
    return Ok((revision(&base)?, MergeKind::AlreadyMerged, None));
  }
  if delivering.is_none() && git.is_ancestor(&base, head)? {
    move_branch(git, (into, title), (&base, head))?;
    return Ok((revision(head)?, MergeKind::FastForward, None));
  }
  let (made, kind) = super::made(title, delivering);
  let (merged, left) = temporary::merge_commit(git, &source.scratch, (&base, head), made)?;
  // Someone may have checked the base branch out while the merge commit was made.
  not_checked_out(git, into)?;
  move_branch(git, (into, title), (&base, &merged))?;
  Ok((revision(&merged)?, kind, left))
}

/// Refuses `branch` while a worktree of the repository has it checked out.
fn not_checked_out<R: CommandRunner>(git: &Git<'_, R>, branch: &BranchName) -> Result<(), MergeError> {
  let listed = git.stdout(&["worktree", "list", "--porcelain", "-z"])?;
  match parse_worktree_list(&listed)
    .into_iter()
    .find(|worktree| !worktree.prunable && worktree.branch.as_ref() == Some(branch))
  {
    Some(worktree) => Err(MergeError::CheckedOutIn {
      branch: branch.clone(),
      path: worktree.path,
    }),
    None => Ok(()),
  }
}

/// Moves `into` from `from` to `to`, logged as `title`; refused, nothing moved, when it is no longer at `from`.
fn move_branch<R: CommandRunner>(
  git: &Git<'_, R>,
  (into, title): (&BranchName, &str),
  (from, to): (&str, &str),
) -> Result<(), MergeError> {
  let reference = format!("refs/heads/{into}");
  let output = git.output(&["update-ref", "-m", title, &reference, to, from])?;
  if output.success() {
    return Ok(());
  }
  if git.commit(&reference)?.as_deref() != Some(from) {
    return Err(MergeError::BaseMoved { branch: into.clone() });
  }
  Err(GitError::Failed(message(&output)).into())
}
