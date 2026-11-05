//! The merge Git made for a run that Orchester could not record (`attempt::AttemptOutcome::NotRecorded`), as *Record
//! merge* on the run's attempt page records it (`crates/desktop-app/design/orchester.pen`, board `Review · redesign`,
//! 09g `i2rVv`): the commit stands in the base branch, so it is recorded as it is and nothing is merged again.

use std::path::Path;

use super::{GitError, MergeKind, git::Git};
use crate::{command::CommandRunner, revision::Revision, run::BranchName};

/// How the commit `made` brought the run's branch `branch` into `base` in the repository at `repo`: a fast-forward when
/// the branch is at it, a merge commit when it joins two parents, else one squashed commit. `None` once `base` no
/// longer holds `made` (it was reset or rewritten since), or there is no `base`: there is nothing to record.
pub fn merge_made(
  runner: &impl CommandRunner,
  git: &Path,
  (repo, base, branch): (&Path, &BranchName, &BranchName),
  made: &Revision,
) -> Result<Option<MergeKind>, GitError> {
  let git = Git::new(runner, git, repo);
  let Some(head) = git.commit(&format!("refs/heads/{base}"))? else {
    return Ok(None);
  };
  if !git.is_ancestor(made.as_str(), &head)? {
    return Ok(None);
  }
  if git.commit(&format!("refs/heads/{branch}"))?.as_deref() == Some(made.as_str()) {
    return Ok(Some(MergeKind::FastForward));
  }
  let parents = git.stdout(&["rev-list", "--parents", "-n", "1", made.as_str()])?;
  Ok(Some(if parents.split_whitespace().count() > 2 {
    MergeKind::MergeCommit
  } else {
    MergeKind::Squash
  }))
}
