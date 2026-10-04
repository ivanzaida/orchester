//! Run again (`docs/design/orchester-workspace.md`, "ORC-16 · Run again"): a task whose latest run
//! failed or was cancelled starts a new run from the first step, in a new worktree on a new branch
//! from the current base, with the latest published version of the task's pipeline and the task's
//! limits. The run that ended keeps its record, branch and (unless a person removes it) worktree.
//! A task has one active run at a time, and new runs wait while today's budget is used. An
//! interrupted run whose worktree a person removed stays interrupted in its record, but it can
//! never continue, so it no longer holds its task (ORC-63): the task runs again after it.

use serde::{Deserialize, Serialize};

use super::{RunStatus, number::RunNumber, state::Run};
use crate::{
  id::RunId,
  limits::ProjectLimits,
  tokens::{TokenBudget, TokensUsed},
};

/// Why a task cannot run again now.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "reason", rename_all = "snake_case")]
pub enum RunAgainBlock {
  TaskBlocked,
  /// The task's latest run neither failed nor was cancelled nor was interrupted and lost its
  /// worktree, or it is not the run the request was made after: only a run that ended that way is
  /// run again.
  NotEnded,
  /// Run `number` of the task has not ended: a task has one active run at a time.
  RunActive {
    number: RunNumber,
  },
  /// The task's pipeline has no published version for the new run to use.
  PipelineNotPublished,
  /// The project's runs used today's token budget: new runs wait until the day has room.
  DailyBudgetUsed {
    used: TokensUsed,
    budget: TokenBudget,
  },
}

/// What deciding whether a task can run again needs.
#[derive(Clone, Copy, Debug)]
pub struct RunAgainInput<'a> {
  /// The task's runs, oldest first; `None` for one whose stored record cannot be read.
  pub runs: &'a [Option<&'a Run>],
  /// The run a person saw end and asks to run again after.
  pub after: RunId,
  /// The task's runs whose own worktree a person removed.
  pub worktree_removed: &'a [RunId],
  /// Whether the task's pipeline has a published version.
  pub pipeline_published: bool,
  pub limits: ProjectLimits,
  /// Tokens the project's runs counted today.
  pub used_today: TokensUsed,
}

/// Whether the task can run again after `input.after`; `Ok` with the run it runs again after.
/// Another active run is named first, since it explains a latest run that is no longer `after`.
pub fn run_again(input: &RunAgainInput<'_>) -> Result<RunId, RunAgainBlock> {
  let removed = |run: &Run| input.worktree_removed.contains(&run.id());
  let active = input
    .runs
    .iter()
    .flatten()
    .filter(|run| holds_task(run, removed(run)))
    .map(|run| run.number())
    .max();
  if let Some(number) = active {
    return Err(RunAgainBlock::RunActive { number });
  }
  let latest = input.runs.last().copied().flatten();
  let ended = latest
    .filter(|run| run.id() == input.after)
    .is_some_and(|run| can_run_again_after(run, removed(run)));
  if !ended {
    return Err(RunAgainBlock::NotEnded);
  }
  if !input.pipeline_published {
    return Err(RunAgainBlock::PipelineNotPublished);
  }
  if input.limits.daily_budget_used(input.used_today) {
    return Err(RunAgainBlock::DailyBudgetUsed {
      used: input.used_today,
      budget: input.limits.daily_budget(),
    });
  }
  Ok(input.after)
}

/// Whether `run`, whose own worktree a person removed when `worktree_removed`, keeps its task from
/// starting another run: it has not ended, unless it was interrupted and lost its worktree.
pub fn holds_task(run: &Run, worktree_removed: bool) -> bool {
  !run.status().is_terminal() && !continues_never(run, worktree_removed)
}

/// Whether the task can run again after `run`, whose own worktree a person removed when
/// `worktree_removed`: it failed or was cancelled, or it was interrupted and lost its worktree.
pub fn can_run_again_after(run: &Run, worktree_removed: bool) -> bool {
  run.status().can_run_again() || continues_never(run, worktree_removed)
}

/// An interrupted run continues in its worktree; once a person removed it, it never does.
fn continues_never(run: &Run, worktree_removed: bool) -> bool {
  worktree_removed && matches!(run.status(), RunStatus::Interrupted(_))
}
