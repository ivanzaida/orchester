//! Changes to runs no task executes: paused, stopped at a limit, interrupted or ended. The
//! runner applies them to the stored run; a run that continues gets a task again.

use std::path::Path;

use orchester_core::{
  id::RunId,
  run::{PauseReason, RaisedLimits, Run, RunError, RunEvent, RunLimit, RunStatus, active_time},
  store::{ActivityStore, RunStore, SettingsStore},
  time::Timestamp,
  tokens::TokenBudget,
};
use orchester_ipc::Rejection;
use orchester_store::{ProjectDb, StoreError};

use super::{failed, prepare::Prepared};

/// A pause of an idle run: a paused run already is, anything else is not running.
pub(super) fn pause(db: &mut ProjectDb, run: RunId) -> Result<(), Rejection> {
  match load(db, run)?.status() {
    RunStatus::Paused(_) => Ok(()),
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
  store(db, &run, &events)?;
  Prepared::continuing(db, root, run)
}

/// Raises the limits of a run stopped at a limit, within the project's maximum run budget, and
/// continues it.
pub(super) fn raise(db: &mut ProjectDb, root: &Path, run: RunId, limits: RaisedLimits) -> Result<Prepared, Rejection> {
  let mut run = load(db, run)?;
  if !matches!(run.status(), RunStatus::Paused(PauseReason::LimitReached(_))) {
    return Err(Rejection::RunNotPaused);
  }
  let now = Timestamp::now();
  let invocations = db
    .invocations(run.id())
    .map_err(|error| failed("cannot read the run's invocations", &error))?;
  let max = db
    .project_limits()
    .map_err(|error| failed("cannot read the project's limits", &error))?
    .max_run_budget();
  let events = run
    .continue_with_limits(limits, active_time(&invocations, now), max, now)
    .map_err(rejection)?;
  store(db, &run, &events)?;
  Prepared::continuing(db, root, run)
}

/// Continues an interrupted run from its last checkpoint because a person asked to.
pub(super) fn continue_interrupted(db: &mut ProjectDb, root: &Path, run: RunId) -> Result<Prepared, Rejection> {
  let mut run = load(db, run)?;
  if !matches!(run.status(), RunStatus::Interrupted(_)) {
    return Err(Rejection::RunNotInterrupted);
  }
  let events = run.continue_run(Timestamp::now()).map_err(rejection)?;
  store(db, &run, &events)?;
  Prepared::continuing(db, root, run)
}

/// Continues a run the previous runner's loss interrupted, from its last checkpoint.
pub(super) fn continue_after_reconnect(db: &mut ProjectDb, root: &Path, run: RunId) -> Result<Prepared, Rejection> {
  let mut run = load(db, run)?;
  let events = run.continue_after_reconnect(Timestamp::now()).map_err(rejection)?;
  if events.is_empty() {
    return Err(Rejection::RunNotInterrupted);
  }
  store(db, &run, &events)?;
  Prepared::continuing(db, root, run)
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

fn load(db: &ProjectDb, run: RunId) -> Result<Run, Rejection> {
  db.run(run)
    .map_err(|error| failed("cannot read the run", &error))?
    .ok_or(Rejection::RunNotFound)
}

fn store(db: &mut ProjectDb, run: &Run, events: &[RunEvent]) -> Result<(), Rejection> {
  db.append_events(run, events)
    .map_err(|error| failed("cannot record the run's change", &error))
}

/// What the desktop reads for a transition the run's state does not allow.
pub(super) fn rejection(error: RunError) -> Rejection {
  match error {
    RunError::LimitNotRaised(limit) => Rejection::LimitNotRaised { limit },
    RunError::LimitNotAboveUsed(limit) => Rejection::LimitNotAboveUsed { limit },
    RunError::BudgetAboveMaximum { max } => Rejection::BudgetAboveMaximum { max },
    other => failed("the run cannot change this way", &other),
  }
}
