//! Changes to runs no task executes: paused, stopped at a limit, interrupted or ended. The
//! runner applies them to the stored run; a run that continues gets a task again.

use std::path::Path;

use orchester_core::{
  id::RunId,
  run::{PauseReason, RaisedLimits, Run, RunError, RunEvent, RunLimit, RunStatus, active_time, execution_tokens},
  store::{ActivityStore, RunStore, SettingsStore},
  time::Timestamp,
  tokens::TokenBudget,
};
use orchester_ipc::Rejection;
use orchester_store::{ProjectDb, RunStoreError, StoreError};

use super::{failed, prepare::Prepared, read_failed};

/// A pause of an idle run: a paused run already is; one waiting for a person's decision has no
/// agent to settle and pauses at once, returned to be announced; anything else is not running.
pub(super) fn pause(db: &mut ProjectDb, run: RunId) -> Result<Option<Run>, Rejection> {
  let mut run = load(db, run)?;
  match run.status() {
    RunStatus::Paused(_) => Ok(None),
    RunStatus::Running if run.waits_for_a_person() => {
      let now = Timestamp::now();
      let mut events = run.request_pause(now).map_err(rejection)?;
      events.extend(run.pause(now).map_err(rejection)?);
      store(db, &run, &events)?;
      Ok(Some(run))
    }
    _ => Err(Rejection::RunNotRunning),
  }
}

/// Cancels an idle run that has not ended; its worktree and branch stay.
pub(super) fn cancel(db: &mut ProjectDb, run: RunId) -> Result<Run, Rejection> {
  let mut run = load(db, run)?;
  if run.status().is_terminal() {
    return Err(Rejection::RunEnded);
  }
  let events = run.cancel(Timestamp::now()).map_err(rejection)?;
  store(db, &run, &events)?;
  Ok(run)
}

/// Resumes a paused run; a run stopped at its own limit needs [`raise`] instead. One stopped by
/// the daily budget resumes as it is once the day has room, which the scheduler checked.
pub(super) fn resume(db: &mut ProjectDb, root: &Path, run: RunId) -> Result<Prepared, Rejection> {
  let mut run = load(db, run)?;
  match run.status() {
    RunStatus::Paused(PauseReason::Requested | PauseReason::LimitReached(RunLimit::DailyBudget)) => {}
    RunStatus::Paused(PauseReason::LimitReached(limit)) => return Err(Rejection::LimitNotRaised { limit: *limit }),
    _ => return Err(Rejection::RunNotPaused),
  }
  let events = run.resume(Timestamp::now()).map_err(rejection)?;
  store_continuation(db, root, run, &events)
}

/// Raises the limits of a run stopped at a limit above what it used, or lifts them, and continues
/// it.
pub(super) fn raise(db: &mut ProjectDb, root: &Path, run: RunId, limits: RaisedLimits) -> Result<Prepared, Rejection> {
  let mut run = load(db, run)?;
  if !matches!(run.status(), RunStatus::Paused(PauseReason::LimitReached(limit)) if limit.is_run_limit()) {
    return Err(Rejection::RunNotPaused);
  }
  let now = Timestamp::now();
  let invocations = db
    .invocations(run.id())
    .map_err(|error| failed("cannot read the run's invocations", &error))?;
  let events = run
    .continue_with_limits(limits, active_time(&invocations, now), now)
    .map_err(rejection)?;
  store_continuation(db, root, run, &events)
}

/// Raises the budget of the step a run stopped at above what the stopped execution used, or lifts
/// it with `None`, and continues the run.
pub(super) fn raise_step_budget(
  db: &mut ProjectDb,
  root: &Path,
  run: RunId,
  budget: Option<TokenBudget>,
) -> Result<Prepared, Rejection> {
  let mut run = load(db, run)?;
  let RunStatus::Paused(PauseReason::LimitReached(RunLimit::StepBudget { execution })) = *run.status() else {
    return Err(Rejection::RunNotPaused);
  };
  let invocations = db
    .invocations(run.id())
    .map_err(|error| failed("cannot read the run's invocations", &error))?;
  let reviews = db
    .reviewer_usage(run.id())
    .map_err(|error| failed("cannot read what the run's reviews cost", &error))?;
  let used = execution_tokens(&invocations, &reviews, execution);
  let events = run
    .raise_step_budget(budget, used, Timestamp::now())
    .map_err(rejection)?;
  store_continuation(db, root, run, &events)
}

/// Continues an interrupted run from its last checkpoint because a person asked to.
pub(super) fn continue_interrupted(db: &mut ProjectDb, root: &Path, run: RunId) -> Result<Prepared, Rejection> {
  let mut run = load(db, run)?;
  if !matches!(run.status(), RunStatus::Interrupted(_)) {
    return Err(Rejection::RunNotInterrupted);
  }
  let events = run.continue_run(Timestamp::now()).map_err(rejection)?;
  store_continuation(db, root, run, &events)
}

/// Continues a run the previous runner's loss interrupted, from its last checkpoint.
pub(super) fn continue_after_reconnect(db: &mut ProjectDb, root: &Path, run: RunId) -> Result<Prepared, Rejection> {
  let mut run = load(db, run)?;
  let events = run.continue_after_reconnect(Timestamp::now()).map_err(rejection)?;
  if events.is_empty() {
    return Err(Rejection::RunNotInterrupted);
  }
  store_continuation(db, root, run, &events)
}

/// A continuation must have a task and worktree before its state change is recorded.
pub(super) fn store_continuation(
  db: &mut ProjectDb,
  root: &Path,
  run: Run,
  events: &[RunEvent],
) -> Result<Prepared, Rejection> {
  let prepared = Prepared::continuing(db, root, run)?;
  store(db, &prepared.run, events)?;
  Ok(prepared)
}

/// Refuses new or continued work while the project's runs have used today's token budget.
pub(super) fn check_daily_budget(db: &ProjectDb) -> Result<(), Rejection> {
  match used_daily_budget(db).map_err(|error| failed("cannot read today's token usage", &error))? {
    Some(budget) => Err(Rejection::DailyBudgetUsed { budget }),
    None => Ok(()),
  }
}

/// Whether the project's runs used today's token budget: today starts at local midnight.
pub(super) fn daily_budget_used(db: &ProjectDb) -> Result<bool, StoreError> {
  Ok(used_daily_budget(db)?.is_some())
}

/// The daily budget, when today's runs used it.
fn used_daily_budget(db: &ProjectDb) -> Result<Option<TokenBudget>, StoreError> {
  let limits = db.project_limits()?;
  let today = db.tokens_used_since(Timestamp::now().local_day_start())?;
  Ok(limits.daily_budget_used(today).then(|| limits.daily_budget()))
}

pub(super) fn load(db: &ProjectDb, run: RunId) -> Result<Run, Rejection> {
  db.run(run)
    .map_err(|error| read_failed("cannot read the run", error))?
    .ok_or(Rejection::RunNotFound)
}

/// Records `events` of `run`; a continuation of a run whose worktrees a person removed meanwhile is
/// refused, the run unchanged.
pub(super) fn store(db: &mut ProjectDb, run: &Run, events: &[RunEvent]) -> Result<(), Rejection> {
  db.append_events(run, events).map_err(|error| match error {
    RunStoreError::WorktreeRemoved(_) => Rejection::WorktreeRemoved,
    other => failed("cannot record the run's change", &other),
  })
}

/// What the desktop reads for a transition the run's state does not allow.
pub(super) fn rejection(error: RunError) -> Rejection {
  match error {
    RunError::LimitNotRaised(limit) => Rejection::LimitNotRaised { limit },
    RunError::LimitNotAboveUsed(limit) => Rejection::LimitNotAboveUsed { limit },
    other => failed("the run cannot change this way", &other),
  }
}
