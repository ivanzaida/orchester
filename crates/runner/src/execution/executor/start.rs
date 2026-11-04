//! The executor's side of a run's start (`docs/architecture/remote-runners.md`, "The split"): the place a recorded run
//! works in, made from the coordinator's [`StartOrder`] — its own worktree at the order's revision, or, for a run that
//! borrows warm workspaces, only its branch — and the place of a run that continues. Only the runner's log has git's
//! full answer.

use std::path::{Path, PathBuf};

use orchester_core::{
  id::RunId,
  run::{BranchName, StartOrder, WorktreeFailure},
};

use super::prepared::RunFolder;
use crate::{
  git::Git,
  worktree::{WorktreeError, Worktrees},
};

/// Where the run of a start order works, found before anything is made.
pub(in crate::execution) struct Place {
  worktrees: Worktrees,
  order: StartOrder,
  folder: RunFolder,
}

impl Place {
  /// The place of `order`'s run in the repository at `root`: its own worktree when an earlier request made it or the
  /// project lends no workspaces (a project that has not turned warm workspaces on gives each run a worktree of its
  /// own, as before them), else the workspaces it borrows.
  pub(in crate::execution) fn find(root: &Path, order: StartOrder) -> Result<Self, WorktreeError> {
    let worktrees = Worktrees::new(root)?;
    let own = worktrees.path(order.run());
    let folder = if own.is_dir() || !order.borrows() {
      RunFolder::Own(own)
    } else {
      RunFolder::Borrowed
    };
    Ok(Self {
      worktrees,
      order,
      folder,
    })
  }

  pub(in crate::execution) const fn folder(&self) -> &RunFolder {
    &self.folder
  }

  pub(in crate::execution) fn into_folder(self) -> RunFolder {
    self.folder
  }

  /// Makes the place and answers with the run's branch: the one its own worktree is on, which is created now at the
  /// order's revision unless an earlier request did; or, for a run that borrows workspaces, a branch made now without
  /// a worktree. Why there is none, when it cannot be.
  pub(in crate::execution) fn make(&self) -> Result<BranchName, WorktreeFailure> {
    let (run, branch, base) = (
      self.order.run(),
      self.order.branch().as_str(),
      self.order.base().as_str(),
    );
    let made = match &self.folder {
      RunFolder::Own(path) if path.is_dir() => self.worktrees.branch_of(run),
      RunFolder::Own(_) => self
        .worktrees
        .create(run, branch, base)
        .map(|created| created.branch().to_owned()),
      RunFolder::Borrowed => self.worktrees.create_branch(branch, base),
    }
    .map_err(|error| {
      tracing::error!(%run, %error, "cannot create the run's branch");
      error.failure()
    })?;
    BranchName::new(&made).map_err(|error| {
      tracing::error!(%run, branch = %made, %error, "the run's worktree is on a branch a run cannot have");
      WorktreeFailure::InvalidBranch
    })
  }
}

/// The folder of `run`'s own worktree in the repository at `root`, whether it is there or not.
pub(in crate::execution) fn worktree_path(root: &Path, run: RunId) -> Result<PathBuf, WorktreeError> {
  Ok(Worktrees::new(root)?.path(run))
}

/// `run`'s own worktree while it is there.
pub(in crate::execution) fn own_worktree(root: &Path, run: RunId) -> Result<Option<PathBuf>, WorktreeError> {
  Ok(Some(worktree_path(root, run)?).filter(|path| path.is_dir()))
}

/// Whether `branch` is still in the repository at `root`: a run that borrows workspaces has all its work there.
pub(in crate::execution) fn branch_kept(root: &Path, branch: &BranchName) -> Result<bool, WorktreeError> {
  let branch = format!("refs/heads/{branch}");
  Git::new(root).succeeds(&["show-ref", "--verify", "--quiet", &branch])
}
