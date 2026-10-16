//! Which branch names a new worktree cannot take: read once for all the numbered names a run's
//! branch may take, and, for a name another creator took after that, told by git's refusal.

use super::WorktreeError;
use crate::git::Git;

/// The branches in the folder of `refs/heads/` that `branch` is in, with one git call: every
/// numbered name of `branch` and the step branches of each are there.
pub(super) fn taken(git: &Git<'_>, branch: &str) -> Result<Vec<String>, WorktreeError> {
  let folder = match branch.rfind('/') {
    Some(end) => format!("refs/heads/{}/", &branch[..end]),
    None => "refs/heads/".to_owned(),
  };
  let listed = git.stdout(&["for-each-ref", "--format=%(refname)", &folder])?;
  Ok(
    listed
      .lines()
      .filter_map(|line| line.strip_prefix("refs/heads/"))
      .map(str::to_owned)
      .collect(),
  )
}

/// Whether a branch of `taken` has the name `name`, is one of its step branches (`<name>--…`),
/// which steps of a run on `name` would make, or is in a folder `name`, where git cannot make it.
pub(super) fn is_taken(taken: &[String], name: &str) -> bool {
  taken.iter().any(|branch| {
    branch
      .strip_prefix(name)
      .is_some_and(|rest| rest.is_empty() || rest.starts_with("--") || rest.starts_with('/'))
  })
}

/// Whether `git branch <branch>` failed, `error`, because another creator took the name since it
/// was read: it made the branch, or holds a ref lock while making or deleting one. Git also says
/// "cannot lock ref" for what lasts (a branch where the name's folder would be, permissions, a full
/// disk), and any other name would fail the same way, so only contention counts.
pub(super) fn taken_meanwhile(branch: &str, error: &WorktreeError) -> bool {
  let WorktreeError::Git { stderr, .. } = error else {
    return false;
  };
  let locked = stderr.contains("Unable to create '") && stderr.contains(".lock'") && stderr.contains("File exists");
  let taken = stderr.contains("already exists") || locked;
  if taken {
    tracing::info!(%branch, %stderr, "the branch name was taken meanwhile; the next one is tried");
  }
  taken
}
