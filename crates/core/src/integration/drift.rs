//! What a run's branch lacks of its base branch (ORC-54, `crate::base_move`): the head the base branch is at in the
//! user's checkout, and for a run whose branch does not hold that head yet, the commits of the base it lacks and the
//! files both sides changed since they forked, which are where merging the base is likely to conflict.

use std::{collections::BTreeSet, path::Path};

use super::{GitError, git::Git};
use crate::{base_move::LandedCommit, command::CommandRunner, revision::Revision, run::BranchName};

/// The most commits a notice lists; the newest come first.
pub const MAX_LANDED: usize = 50;

/// What a run's branch lacks of its base branch's head.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Drift {
  /// The commits of the base the branch lacks, newest first, at most [`MAX_LANDED`].
  pub commits: Vec<LandedCommit>,
  /// The files changed on both sides since they forked, in path order.
  pub overlapping: Vec<String>,
}

/// The commit `branch` points at in the repository at `repo`; `None` when there is no such branch.
pub fn branch_head(
  runner: &impl CommandRunner,
  git: &Path,
  (repo, branch): (&Path, &BranchName),
) -> Result<Option<Revision>, GitError> {
  Git::new(runner, git, repo)
    .commit(&format!("refs/heads/{branch}"))?
    .map(|sha| revision(&sha))
    .transpose()
}

/// What `branch` lacks of `head` in the repository at `repo`; `None` when it holds `head` already, or when there is
/// no such branch (the run never made it, or it was deleted).
pub fn drift(
  runner: &impl CommandRunner,
  git: &Path,
  (repo, branch, head): (&Path, &BranchName, &Revision),
) -> Result<Option<Drift>, GitError> {
  let git = Git::new(runner, git, repo);
  let Some(tip) = git.commit(&format!("refs/heads/{branch}"))? else {
    return Ok(None);
  };
  if git.is_ancestor(head.as_str(), &tip)? {
    return Ok(None);
  }
  let fork = git.stdout(&["merge-base", head.as_str(), &tip])?;
  let listed = git.stdout(&[
    "log",
    "--format=%H%x09%s",
    &format!("--max-count={MAX_LANDED}"),
    &format!("{tip}..{head}"),
  ])?;
  let commits = listed
    .lines()
    .filter_map(|line| {
      let (sha, subject) = line.split_once('\t')?;
      Some(LandedCommit {
        revision: Revision::new(sha.trim()).ok()?,
        subject: subject.to_owned(),
      })
    })
    .collect();
  let theirs = changed(&git, &fork, head.as_str())?;
  let ours = changed(&git, &fork, &tip)?;
  Ok(Some(Drift {
    commits,
    overlapping: ours.intersection(&theirs).cloned().collect(),
  }))
}

/// The files that differ between `from` and `to`.
fn changed<R: CommandRunner>(git: &Git<'_, R>, from: &str, to: &str) -> Result<BTreeSet<String>, GitError> {
  let listed = git.stdout(&["-c", "core.quotepath=false", "diff", "--name-only", from, to])?;
  Ok(
    listed
      .lines()
      .map(str::trim)
      .filter(|path| !path.is_empty())
      .map(str::to_owned)
      .collect(),
  )
}

fn revision(sha: &str) -> Result<Revision, GitError> {
  Revision::new(sha).map_err(|_| GitError::Unrecognized {
    expected: "a commit",
    detail: sha.to_owned(),
  })
}
