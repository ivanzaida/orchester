//! Runs stopped at their account's plan limit (`docs/design/agent-accounts.md`, "Plan limits and the limit-reached
//! behaviour"). A running agent never moves to another account by itself: the run stops at the CLI's refusal with a
//! checkpoint and waits. At the limit's reset the runner continues it, the same session on the same account; if the
//! CLI refuses again, it stops again. A person may instead continue the stopped step now on another connection of
//! the same CLI, which runs on another account, for this run only: a new session there from the checkpoint, as a
//! session lives in its account's folder.

use std::{
  collections::HashSet,
  path::Path,
  time::{Duration, SystemTime, UNIX_EPOCH},
};

use orchester_core::{
  connection::{AgentConnection, invocation::Platform},
  id::{ConnectionId, RunId},
  resource::{AgentProfile, StepKey},
  run::{PauseReason, Run, RunLimit, RunStatus, StepExecutor},
  store::{ConnectionStore, ResourceStore},
  time::Timestamp,
};
use orchester_ipc::Rejection;
use orchester_store::ProjectDb;
use parking_lot::Mutex;

use super::{
  Scheduler, failed,
  idle::{load, rejection, store_continuation},
  prepare::Prepared,
};

/// A little after the reset, so the provider counts the window as new.
const AFTER_RESET: Duration = Duration::from_secs(30);

/// The runs a timer waits for, so each gets one.
#[derive(Default)]
pub(super) struct PlanWaits(Mutex<HashSet<RunId>>);

/// Continues `run`, stopped at its account's plan limit, on `connection`: another account's connection of the stopped
/// step's CLI that can start now. Recorded for this run's step only; the profile keeps its own connection.
pub(super) fn continue_on(
  db: &mut ProjectDb,
  root: &Path,
  run: RunId,
  connection: ConnectionId,
) -> Result<Prepared, Rejection> {
  let mut run = load(db, run)?;
  let RunStatus::Paused(PauseReason::LimitReached(RunLimit::PlanLimit { execution })) = *run.status() else {
    return Err(Rejection::RunNotPaused);
  };
  let step = run
    .execution(execution)
    .expect("a run stopped at a plan limit names one of its executions")
    .step()
    .clone();
  let stopped = step_connection(db, &run, &step)?;
  let chosen = db
    .connection(connection)
    .map_err(|error| failed("cannot read the connection", &error))?
    .and_then(|chosen| AgentConnection::try_from(chosen).ok())
    .ok_or(Rejection::NotAnotherAccount)?;
  let provider = stopped.provider();
  let another = chosen.provider() == provider
    && chosen.account().id(provider) != stopped.account().id(provider)
    && chosen.status().is_usable()
    && chosen
      .account()
      .start_problem(provider, Platform::current(), Path::is_dir)
      .is_none();
  if !another {
    return Err(Rejection::NotAnotherAccount);
  }
  db.set_step_connection(run.id(), &step, connection)
    .map_err(|error| failed("cannot record the step's connection", &error))?;
  let events = run.resume(Timestamp::now()).map_err(rejection)?;
  store_continuation(db, root, run, &events)
}

/// The agent connection `step` of `run` runs on now: one chosen for this run, else its pinned profile's.
fn step_connection(db: &ProjectDb, run: &Run, step: &StepKey) -> Result<AgentConnection, Rejection> {
  let chosen = db
    .step_connection(run.id(), step)
    .map_err(|error| failed("cannot read the step's connection", &error))?;
  let id = match chosen {
    Some(id) => id,
    None => {
      let Some(StepExecutor::Agent { profile, .. }) = run.snapshot().step(step).map(|step| step.executor()) else {
        return Err(Rejection::RunNotPaused);
      };
      db.version::<AgentProfile>(profile.resource, profile.number)
        .map_err(|error| failed("cannot read the step's agent profile", &error))?
        .ok_or(Rejection::RunNotPaused)?
        .body()
        .connection()
    }
  };
  db.connection(id)
    .map_err(|error| failed("cannot read the step's connection", &error))?
    .and_then(|connection| AgentConnection::try_from(connection).ok())
    .ok_or(Rejection::NotAnotherAccount)
}

impl Scheduler {
  /// Gives every run stopped at a plan limit a timer that continues it at the limit's reset, unless it has one: when
  /// the runner starts and whenever a run's task ends. A run whose CLI did not say when its limit resets waits for a
  /// person.
  pub(super) fn wait_for_plan_resets(&self) {
    let db = match self.open() {
      Ok(db) => db,
      Err(rejection) => {
        tracing::warn!(?rejection, "cannot look for runs waiting for a plan limit's reset");
        return;
      }
    };
    let runs = match db.runs_at_plan_limit() {
      Ok(runs) => runs,
      Err(error) => {
        tracing::warn!(%error, "cannot look for runs waiting for a plan limit's reset");
        return;
      }
    };
    let now = Timestamp::now();
    // A run that cannot be read is shown as unreadable by the task screens; it waits for a person.
    for run in runs.into_iter().filter_map(Result::ok) {
      let reset = match db.last_limit_reading(run.id()) {
        Ok(reading) => reading.and_then(|reading| reading.resets_at(now)),
        Err(error) => {
          tracing::warn!(%error, run = %run.id(), "cannot read when a run's plan limit resets");
          None
        }
      };
      if let Some(reset) = reset {
        self.wait_for_reset(run.id(), reset);
      }
    }
  }

  /// Continues `run` once `reset` has passed, as resuming it does.
  fn wait_for_reset(&self, run: RunId, reset: Timestamp) {
    if !self.plan_waits().0.lock().insert(run) {
      return;
    }
    let scheduler = self.clone();
    let wait = until(reset) + AFTER_RESET;
    drop(tokio::spawn(async move {
      tokio::time::sleep(wait).await;
      scheduler.plan_waits().0.lock().remove(&run);
      match scheduler.resume_run(run).await {
        Ok(()) => tracing::info!(%run, "continued a run at its account's plan limit reset"),
        // A person continued or ended it meanwhile, or the runner cannot take it now: it stays as it is.
        Err(rejection) => tracing::info!(%run, ?rejection, "a run waiting for a plan limit's reset was not continued"),
      }
    }));
  }
}

/// How long until `at`; nothing once it has passed.
fn until(at: Timestamp) -> Duration {
  let at = UNIX_EPOCH + Duration::from_millis(u64::try_from(at.unix_millis()).unwrap_or_default());
  at.duration_since(SystemTime::now()).unwrap_or_default()
}
