//! A test seam for the runs made before warm workspaces (ORC-114), built only with the `test-agent` feature: a
//! scheduler [`Scheduler::with_own_worktrees`] gives every run it starts a worktree of its own,
//! `.orchester/worktrees/<run>`, as runs had before, so the tests of what such runs do — their worktree removed, steps
//! beside others in worktrees of their own — keep covering that path, which runs that already work go on with.

use std::sync::atomic::Ordering;

use orchester_core::store::WorkspaceStore;
use orchester_ipc::Rejection;

use super::{
  Scheduler, failed,
  prepare::{Prepared, RunFolder},
};
use crate::worktree::Worktrees;

impl Scheduler {
  /// This scheduler giving each run its own worktree instead of a borrowed workspace.
  #[must_use]
  pub fn with_own_worktrees(self) -> Self {
    self.inner.own_worktrees.store(true, Ordering::Release);
    self
  }

  /// `prepared` with a worktree of its own on its branch, when this scheduler gives runs one and it borrows; the run
  /// is recorded as one that does not borrow, as a run made before workspaces.
  pub(super) fn own_worktree_for_tests(&self, mut prepared: Prepared) -> Result<Prepared, Rejection> {
    if !self.inner.own_worktrees.load(Ordering::Acquire) || prepared.folder != RunFolder::Borrowed {
      return Ok(prepared);
    }
    let worktrees =
      Worktrees::new(&self.inner.project_root).map_err(|error| failed("cannot find the worktrees", &error))?;
    let path = worktrees
      .add_on_branch(prepared.run.id(), prepared.run.branch().as_str())
      .map_err(|error| failed("cannot make the run's own worktree", &error))?;
    self
      .open()?
      .set_run_borrows(prepared.run.id(), false)
      .map_err(|error| failed("cannot record that the run has its own worktree", &error))?;
    prepared.folder = RunFolder::Own(path);
    Ok(prepared)
  }
}
