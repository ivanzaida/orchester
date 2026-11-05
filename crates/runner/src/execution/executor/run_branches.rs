//! The tips of the runs' branches, which the executor's worktrees are on (`docs/architecture/remote-runners.md`, "The
//! split"), as the runner starts: which run's branch went ahead of the revision the run started from.

use std::{collections::HashMap, path::Path};

use orchester_core::{revision::Revision, run::BranchName};

use crate::{git::Git, worktree::WorktreeError};

/// The tip of each local branch of a repository, read once.
pub(in crate::execution) struct BranchTips<'a> {
  git: Git<'a>,
  tips: HashMap<String, Revision>,
}

impl<'a> BranchTips<'a> {
  /// The tip of each local branch of the repository at `root`, by its name.
  pub(in crate::execution) fn read(root: &'a Path) -> Result<Self, WorktreeError> {
    let git = Git::new(root);
    let listed = git.stdout(&["for-each-ref", "--format=%(objectname) %(refname)", "refs/heads"])?;
    let tips = listed
      .lines()
      .filter_map(|line| {
        let (revision, name) = line.split_once(' ')?;
        let name = name.strip_prefix("refs/heads/")?;
        Some((name.to_owned(), Revision::new(revision).ok()?))
      })
      .collect();
    Ok(Self { git, tips })
  }

  /// The tip of `branch` when it holds commits on top of `base`; `None` when the branch is gone, still at `base` or
  /// elsewhere.
  pub(in crate::execution) fn ahead_of(
    &self,
    branch: &BranchName,
    base: &Revision,
  ) -> Result<Option<&Revision>, WorktreeError> {
    let Some(tip) = self.tips.get(branch.as_str()) else {
      return Ok(None);
    };
    if tip == base
      || !self
        .git
        .answer(&["merge-base", "--is-ancestor", base.as_str(), tip.as_str()])?
    {
      return Ok(None);
    }
    Ok(Some(tip))
  }
}
