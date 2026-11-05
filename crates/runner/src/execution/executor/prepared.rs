//! What the coordinator hands the executor for a run it recorded (`super::super::coordinator::prepare`): the run, its
//! task and where it works.

use std::path::PathBuf;

use orchester_core::{id::StepExecutionId, run::Run, task::Task};

use super::resolver::RunResolver;

/// A recorded run and whether this request is the one that executes it.
pub(in crate::execution) struct Prepared {
  pub(in crate::execution) run: Run,
  /// The run's task as the run started it.
  pub(in crate::execution) task: Task,
  /// Where the run's pinned configuration is read from.
  pub(in crate::execution) resolver: RunResolver,
  pub(in crate::execution) folder: RunFolder,
  /// `false` when an earlier request with the same key already started the run.
  pub(in crate::execution) execute: bool,
  /// A step a person asked to start on its account now, though the account's last reading says its plan limit
  /// holds: it starts once, and stops again if the CLI refuses.
  pub(in crate::execution) try_at_plan_limit: Option<StepExecutionId>,
}

/// Where a run works.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::execution) enum RunFolder {
  /// Its own worktree, `.orchester/worktrees/<run>`, made before workspaces (ORC-114): it works there until it ends.
  Own(PathBuf),
  /// A warm workspace it borrows when its next step starts and gives back when it waits or ends.
  Borrowed,
}
