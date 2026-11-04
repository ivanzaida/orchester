//! A team module's branch delivered to base (team authority S7c, the server's `docs/module-branches.md`): the module
//! branch at the head the server authorized merged into the base branch only while that branch is at the base commit
//! the server authorized, by the merge a run takes. It always makes a merge commit (as the merge queue does), so the
//! commit it leaves names both authorized commits as its parents, in the checkout while the checkout has the base
//! branch checked out, else beside it. A branch or a base that moved since merges nothing.

use std::path::Path;

use super::{DeliveredAs, Delivering, MergeError, MergeKind, Source, TemporaryLeft, merge_source, revision};
use crate::{
  command::CommandRunner,
  id::ModuleId,
  integration::{GitError, base::Landing, git::Git},
  revision::Revision,
  run::BranchName,
};

/// What a module delivery merges: `module`'s `branch` at `head` into `into` at `base`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ModuleDelivery<'a> {
  pub module: ModuleId,
  pub branch: &'a BranchName,
  pub head: &'a Revision,
  pub into: &'a BranchName,
  pub base: &'a Revision,
}

/// A module delivery that merged: the commit its base branch is at now, how, and the temporary worktree of the merge
/// commit when it could not be removed (the merge stands either way).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModuleMerge {
  pub revision: Revision,
  pub kind: MergeKind,
  pub left: Option<TemporaryLeft>,
}

/// Merges `delivery` in the repository at `repo`, with the `git` program.
pub fn merge_module(
  runner: &impl CommandRunner,
  (git, repo): (&Path, &Path),
  delivery: &ModuleDelivery<'_>,
) -> Result<ModuleMerge, MergeError> {
  let source = Source {
    branch: delivery.branch,
    into: delivery.into,
    title: format!("Merge {}", delivery.branch),
    scratch: format!("module-{}", delivery.module),
  };
  let delivering = Delivering {
    as_delivered: DeliveredAs::MergeCommit,
    base: delivery.base,
  };
  let landed = (Some(delivery.head), Some(delivering), Landing::BesideOtherBranch);
  let (revision, kind, left) = merge_source(runner, (git, repo), &source, landed)?;
  Ok(ModuleMerge { revision, kind, left })
}

/// The merge commit an earlier [`merge_module`] of `delivery` left: a merge commit of exactly the base and the head, in
/// that order, that the base branch contains, even when commits landed on it since; `None` when it contains none. A
/// merge whose record was lost is found by it, so it is never made twice.
pub fn module_merged(
  runner: &impl CommandRunner,
  (git, repo): (&Path, &Path),
  delivery: &ModuleDelivery<'_>,
) -> Result<Option<Revision>, GitError> {
  let git = Git::new(runner, git, repo);
  let Some(tip) = git.commit(&format!("refs/heads/{}", delivery.into))? else {
    return Ok(None);
  };
  // A base this repository does not have was never merged into: git refuses to list the commits since it.
  if git.commit(delivery.base.as_str())?.is_none() {
    return Ok(None);
  }
  let since = format!("^{}", delivery.base.as_str());
  let listed = git.stdout(&["rev-list", "--merges", "--parents", &tip, &since])?;
  let parents = format!("{} {}", delivery.base.as_str(), delivery.head.as_str());
  let found = listed
    .lines()
    .find_map(|line| line.split_once(' ').filter(|(_, listed)| *listed == parents));
  found.map(|(commit, _)| revision(commit)).transpose()
}
