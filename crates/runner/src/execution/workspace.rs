//! Where a step execution works (`docs/architecture/storage.md`, "Agent worktrees").
//!
//! Parallel steps writing to one worktree would overwrite each other's files, and a merge into a
//! worktree an agent is working in would change files under it. So:
//!
//! - A step that starts alone — no other step of the run is running and no other one is ready — works in the run's
//!   worktree, on the run's branch, as a sequential pipeline always has. Nothing else can start while it runs: only a
//!   passing step makes another one ready.
//! - A step that starts beside others, write or read-only alike, works in a worktree of its own on a branch off the
//!   run's branch as it is when the step starts, which holds every step it depends on. When it passes, its branch is
//!   merged back into the run's branch in the run's worktree, one merge at a time, and its worktree and branch are
//!   removed. A conflict fails the step with the files named; its branch and worktree stay for inspection.
//!
//! Which one an execution uses is recorded by the worktree itself: an execution with a worktree of
//! its own works there, also when it continues after a pause, a limit or an interruption.

use std::path::{Path, PathBuf};

use orchester_core::{id::StepExecutionId, run::Run};

use super::checkpoint;
use crate::worktree::{WorktreeError, Worktrees};

/// How an execution starts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Start {
  /// New, with no other step of the run running or ready.
  Alone,
  /// New, while other steps of the run run or are ready to.
  BesideOthers,
  /// It started earlier and continues where it worked before.
  Continues,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Workspace {
  /// The run's worktree, on the run's branch: the step runs alone.
  Run(PathBuf),
  /// A worktree of the step's own, on `branch` off the run's branch.
  Own {
    path: PathBuf,
    branch: String,
    owner: StepExecutionId,
  },
}

impl Workspace {
  pub(super) fn path(&self) -> &Path {
    match self {
      Self::Run(path) | Self::Own { path, .. } => path,
    }
  }

  /// Where `execution` of `run` works when it starts as `start` says. A new execution beside
  /// others gets a worktree of its own, created now from the head of the run's worktree
  /// `run_worktree`.
  pub(super) fn of(
    root: &Path,
    run: &Run,
    run_worktree: &Path,
    execution: StepExecutionId,
    start: Start,
  ) -> Result<Self, WorktreeError> {
    let worktrees = Worktrees::new(root)?;
    let mut previous = run.recovered_execution(execution);
    while let Some(earlier) = previous {
      let path = worktrees.step_path(earlier);
      if path.is_dir() {
        return Ok(Self::Own {
          path,
          branch: branch(run, earlier),
          owner: earlier,
        });
      }
      previous = run.recovered_execution(earlier);
    }
    let path = worktrees.step_path(execution);
    let branch = branch(run, execution);
    if path.is_dir() {
      return Ok(Self::Own {
        path,
        branch,
        owner: execution,
      });
    }
    if start != Start::BesideOthers {
      return Ok(Self::Run(run_worktree.to_path_buf()));
    }
    let base = checkpoint::head(run_worktree)?;
    worktrees.create_step(execution, &branch, base.as_str())?;
    Ok(Self::Own {
      path,
      branch,
      owner: execution,
    })
  }
}

/// The branch of a step working beside others: `orchester/orc-12-run-1--review-2` for step
/// `review` in attempt 2.
fn branch(run: &Run, execution: StepExecutionId) -> String {
  let step = run
    .execution(execution)
    .map_or_else(String::new, |execution| execution.step().to_string());
  let attempt = run
    .attempts()
    .iter()
    .find(|attempt| attempt.execution(execution).is_some())
    .map_or(0, |attempt| attempt.number().get());
  format!("{}--{step}-{attempt}", run.branch())
}
