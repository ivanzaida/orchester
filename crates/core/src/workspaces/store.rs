//! Where the workspaces are kept, a persistence boundary like the others of [`crate::store`].

use std::num::NonZeroU32;

use super::{
  WorkspaceNumber,
  pool::{Borrow, Holder, Stranded, WorkspaceRecord},
  settings::{WarmUpFailure, WorkspaceSettings},
};
use crate::{id::RunId, run::BranchName, time::Timestamp};

/// The project's warm workspaces: who holds each, the runs waiting for one, and what the person set for them. The
/// runner and the app both borrow (the merge queue's re-check builds too), so a borrow decides with
/// [`super::choose`] and records the result in one write.
pub trait WorkspaceStore {
  type Error: std::error::Error + Send + Sync + 'static;

  /// Every workspace made so far, by number.
  fn workspaces(&self) -> Result<Vec<WorkspaceRecord>, Self::Error>;

  /// Takes a workspace for `holder`, to work on `branch` (`None`: the base branch), with `limit` agents at once, and
  /// records it held since `now`, its run no longer waiting; or, with none free, records that its run waits since
  /// `now` (a wait recorded before keeps its time).
  fn borrow_workspace(
    &mut self,
    holder: Holder,
    branch: Option<&BranchName>,
    limit: NonZeroU32,
    now: Timestamp,
  ) -> Result<Borrow, Self::Error>;

  /// Gives back the workspace `holder` holds, idle and warm since `now`; `None` when it holds none.
  fn return_workspace(&mut self, holder: Holder, now: Timestamp) -> Result<Option<WorkspaceNumber>, Self::Error>;

  /// Gives back every workspace a runner's work held — runs, their steps and the warm-up — when a runner starts and
  /// none of it works any more. The merge queue's re-checks and a person's upkeep are the app's and stay.
  fn return_runner_workspaces(&mut self, now: Timestamp) -> Result<Vec<WorkspaceNumber>, Self::Error>;

  /// Takes workspace `number` for `holder` if it is idle, in one write, so no borrower takes it meanwhile; `false`
  /// when somebody holds it, or it was never made.
  fn reserve_idle_workspace(&mut self, number: WorkspaceNumber, holder: Holder) -> Result<bool, Self::Error>;

  /// Frees workspace `number` from `holder`, keeping when it was last used and its warmth: what
  /// [`WorkspaceStore::reserve_idle_workspace`] took.
  fn release_workspace(&mut self, number: WorkspaceNumber, holder: Holder) -> Result<(), Self::Error>;

  /// Records that `number` holds `stranded`, which nobody takes until a person keeps or discards them; `None` once
  /// they did.
  fn set_workspace_stranded(&mut self, number: WorkspaceNumber, stranded: Option<Stranded>) -> Result<(), Self::Error>;

  /// Records that `number` lost what it had built: cleanup deleted its warm folders.
  fn set_workspace_cold(&mut self, number: WorkspaceNumber) -> Result<(), Self::Error>;

  /// Forgets that `run` waits for a workspace: it got one, or no longer needs one.
  fn end_workspace_wait(&mut self, run: RunId) -> Result<(), Self::Error>;

  /// Records whether `run` borrows workspaces, as every run made since them does; one that does not has a worktree
  /// of its own.
  fn set_run_borrows(&mut self, run: RunId, borrows: bool) -> Result<(), Self::Error>;

  /// Whether `run` borrows workspaces.
  fn run_borrows(&self, run: RunId) -> Result<bool, Self::Error>;

  /// The runs waiting for a workspace, with since when, oldest first.
  fn workspace_waits(&self) -> Result<Vec<(RunId, Timestamp)>, Self::Error>;

  /// The project's workspace settings: the defaults until they were changed.
  fn workspace_settings(&self) -> Result<WorkspaceSettings, Self::Error>;

  fn save_workspace_settings(&mut self, settings: &WorkspaceSettings) -> Result<(), Self::Error>;

  /// The last warm-up that failed, until one succeeds.
  fn warm_up_failure(&self) -> Result<Option<WarmUpFailure>, Self::Error>;

  /// Records `failure`, or with `None` that a warm-up succeeded.
  fn save_warm_up_failure(&mut self, failure: Option<&WarmUpFailure>) -> Result<(), Self::Error>;
}
