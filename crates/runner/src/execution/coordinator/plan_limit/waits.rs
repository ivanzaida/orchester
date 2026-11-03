//! The timers that continue runs stopped at their account's plan limit at the limit's reset: one per stop (the run,
//! its stopped execution and the reset the CLI reported), replaced when the run stops again and dropped when a person
//! continues, resumes or cancels the run. A timer continues only the stop it was set for: a run a person paused or
//! continued meanwhile, or that stopped again elsewhere, stays as it is. A reset that passed while no runner ran
//! continues the run as soon as the runner starts.

use std::{
  collections::HashMap,
  path::Path,
  time::{Duration, SystemTime, UNIX_EPOCH},
};

use orchester_core::{
  id::{RunId, StepExecutionId},
  run::{PauseReason, Run, RunLimit, RunStatus},
  time::Timestamp,
};
use orchester_ipc::Rejection;
use orchester_store::ProjectDb;
use parking_lot::Mutex;
use tokio::task::AbortHandle;

use super::super::{Scheduler, failed, idle, prepare::Prepared};

/// A little after the reset, so the provider counts the window as new.
const AFTER_RESET: Duration = Duration::from_secs(30);

/// A run's stop at its account's plan limit, as its timer waits for it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Stop {
  execution: StepExecutionId,
  reset: Timestamp,
}

/// The timer of each run stopped at a plan limit.
#[derive(Default)]
pub(in crate::execution) struct PlanWaits(Mutex<HashMap<RunId, (Stop, AbortHandle)>>);

impl PlanWaits {
  /// Drops `run`'s timer: a person continued, resumed or cancelled it.
  pub(in crate::execution) fn clear(&self, run: RunId) {
    if let Some((_, timer)) = self.0.lock().remove(&run) {
      timer.abort();
    }
  }

  /// Takes `run`'s timer out for `stop` as it fires; `false` when it was replaced or dropped meanwhile.
  fn fired(&self, run: RunId, stop: Stop) -> bool {
    let mut waits = self.0.lock();
    if waits.get(&run).is_some_and(|(waiting, _)| *waiting == stop) {
      waits.remove(&run);
      return true;
    }
    false
  }
}

impl Scheduler {
  /// Gives every run stopped at a plan limit a timer for its stop, replacing one set for an earlier stop and dropping
  /// those of runs no longer stopped: when the runner starts and whenever a run's task ends. A run whose CLI did not
  /// say when its limit resets waits for a person.
  pub(in crate::execution) fn wait_for_plan_resets(&self) {
    let stops = match self.open().map(|db| stops(&db)) {
      Ok(Ok(stops)) => stops,
      Ok(Err(error)) => {
        tracing::warn!(%error, "cannot look for runs waiting for a plan limit's reset");
        return;
      }
      Err(rejection) => {
        tracing::warn!(?rejection, "cannot look for runs waiting for a plan limit's reset");
        return;
      }
    };
    let mut waits = self.plan_waits().0.lock();
    waits.retain(|run, (stop, timer)| {
      let keep = stops.get(run) == Some(stop);
      if !keep {
        timer.abort();
      }
      keep
    });
    for (run, stop) in stops {
      waits
        .entry(run)
        .or_insert_with(|| (stop, self.continue_at_reset(run, stop)));
    }
  }

  /// Keeps `run`, stopped at its account's plan limit, waiting for the reset, as the person confirmed: its timer is
  /// set (or kept). Refused when it is not stopped at a plan limit whose reset its CLI reported.
  pub fn wait_for_plan_reset(&self, run: RunId) -> Result<(), Rejection> {
    let db = self.open()?;
    let current = idle::load(&db, run)?;
    let stop = stop_of(&db, &current).map_err(|error| failed("cannot read the run's stop", &error))?;
    if stop.is_none() {
      return Err(Rejection::RunNotPaused);
    }
    self.wait_for_plan_resets();
    Ok(())
  }

  /// Whether `run` is no longer at `stop`: it stopped again elsewhere, or a person continued or ended it.
  fn stop_changed(&self, run: RunId, stop: Stop) -> bool {
    let current = self
      .open()
      .and_then(|db| idle::load(&db, run).map(|current| (db, current)))
      .ok()
      .and_then(|(db, current)| stop_of(&db, &current).ok());
    // A run that can't be read now keeps its timer as it is: nothing new to wait for.
    current.is_some_and(|current| current != Some(stop))
  }

  /// A timer that continues `run` at `stop`'s reset, if it is still stopped there then.
  fn continue_at_reset(&self, run: RunId, stop: Stop) -> AbortHandle {
    let scheduler = self.clone();
    let wait = if stop.reset <= Timestamp::now() {
      Duration::ZERO
    } else {
      until(stop.reset) + AFTER_RESET
    };
    tokio::spawn(async move {
      tokio::time::sleep(wait).await;
      if !scheduler.plan_waits().fired(run, stop) {
        return;
      }
      let continued = scheduler
        .continue_settled(run, |db, root| resume_at_reset(db, root, run, stop))
        .await;
      match continued {
        Ok(()) => tracing::info!(%run, "continued a run at its account's plan limit reset"),
        // A person continued, paused or ended it meanwhile, or it stopped again elsewhere (another account after its
        // connection changed): it stays as it is, and a stop it is at now gets its own timer.
        // Only a different stop: the same one refused again would fire again at once.
        Err(rejection) => {
          tracing::info!(%run, ?rejection, "a run waiting for a plan limit's reset was not continued");
          if scheduler.stop_changed(run, stop) {
            scheduler.wait_for_plan_resets();
          }
        }
      }
    })
    .abort_handle()
  }
}

/// Each run stopped at a plan limit whose CLI said when the limit resets, with its stop.
fn stops(db: &ProjectDb) -> Result<HashMap<RunId, Stop>, orchester_store::RunStoreError> {
  let mut stops = HashMap::new();
  // A run that cannot be read is shown as unreadable by the task screens; it waits for a person.
  for run in db.runs_at_plan_limit()?.into_iter().filter_map(Result::ok) {
    if let Some(stop) = stop_of(db, &run)? {
      stops.insert(run.id(), stop);
    }
  }
  Ok(stops)
}

/// The stop of `run`: `None` unless it is stopped at a plan limit whose reset the CLI reported. A step that waits
/// before its agent starts waits for the account its connection runs on.
fn stop_of(db: &ProjectDb, run: &Run) -> Result<Option<Stop>, orchester_store::StoreError> {
  let RunStatus::Paused(PauseReason::LimitReached(RunLimit::PlanLimit { execution })) = *run.status() else {
    return Ok(None);
  };
  let step_account = match run.execution(execution) {
    Some(stopped) => db
      .step_agent_connection(run, stopped.step())?
      .and_then(|connection| connection.plan_account()),
    None => None,
  };
  let reset = db
    .plan_stop_reading(run.id(), execution, step_account)?
    .and_then(|reading| reading.reached_limit_resets());
  Ok(reset.map(|reset| Stop { execution, reset }))
}

/// Resumes `run` if it is still at `stop`: the same session continues on the same account.
fn resume_at_reset(db: &mut ProjectDb, root: &Path, run: RunId, stop: Stop) -> Result<Prepared, Rejection> {
  let current = idle::load(db, run)?;
  let still = stop_of(db, &current).map_err(|error| failed("cannot read the run's stop", &error))?;
  if still != Some(stop) {
    return Err(Rejection::RunNotPaused);
  }
  idle::resume(db, root, run)
}

/// How long until `at`; nothing once it has passed.
fn until(at: Timestamp) -> Duration {
  let at = UNIX_EPOCH + Duration::from_millis(u64::try_from(at.unix_millis()).unwrap_or_default());
  at.duration_since(SystemTime::now()).unwrap_or_default()
}
