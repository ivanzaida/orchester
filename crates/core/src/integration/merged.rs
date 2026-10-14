//! Which branches a base branch already holds: a run whose work a person merged outside Orchester, with
//! git or through a pull request that came back into the checkout, needs no merge any more.

use std::{collections::HashSet, path::Path};

use super::{GitError, git::Git};
use crate::{command::CommandRunner, run::BranchName};

const HEADS: &str = "refs/heads/";

/// The local branches of the repository at `repo` whose tip `into` already holds, read with the `git`
/// program in one `git for-each-ref`; `into` itself among them. Empty when `into` names no branch.
pub fn merged_into(
  runner: &impl CommandRunner,
  git: &Path,
  repo: &Path,
  into: &BranchName,
) -> Result<HashSet<BranchName>, GitError> {
  let git = Git::new(runner, git, repo);
  let target = format!("{HEADS}{into}");
  if git.commit(&target)?.is_none() {
    return Ok(HashSet::new());
  }
  let listed = git.stdout(&[
    "for-each-ref",
    "--format=%(refname)",
    &format!("--merged={target}"),
    HEADS,
  ])?;
  Ok(
    listed
      .lines()
      .filter_map(|line| line.trim().strip_prefix(HEADS))
      // A name git allows but Orchester's branch names do not cannot be a run's branch.
      .filter_map(|name| BranchName::new(name).ok())
      .collect(),
  )
}
