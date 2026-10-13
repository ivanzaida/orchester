//! Runs stopped at their account's plan limit (`docs/design/agent-accounts.md`, "Plan limits and the limit-reached
//! behaviour"). A running agent never moves to another account by itself: the run stops at the CLI's refusal with a
//! checkpoint and waits. At the limit's reset the runner continues it, the same session on the same account; if the
//! CLI refuses again, it stops again. A person may instead continue the stopped step now on another connection of
//! the same CLI, which runs on another account, for this run only: a new session there from the checkpoint, as a
//! session lives in its account's folder.

mod waits;

use std::path::Path;

use orchester_core::{
  connection::{AgentConnection, invocation::Platform},
  id::{ConnectionId, RunId},
  run::{PauseReason, RunLimit, RunStatus},
  store::ConnectionStore,
  time::Timestamp,
};
use orchester_ipc::Rejection;
use orchester_store::ProjectDb;
pub(super) use waits::PlanWaits;

use super::{
  failed,
  idle::{load, rejection, store_continuation},
  prepare::Prepared,
};

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
  let stopped = db
    .step_agent_connection(&run, &step)
    .map_err(|error| failed("cannot read the step's connection", &error))?
    .ok_or(Rejection::NotAnotherAccount)?;
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

/// Continues `run`, stopped at its account's plan limit, on the same account now, as a person asked: a step whose
/// agent waited starts once though the account's last reading says the limit holds, and a stopped agent continues its
/// session. If the CLI refuses at the limit, the run stops there again.
pub(super) fn try_now(db: &mut ProjectDb, root: &Path, run: RunId) -> Result<Prepared, Rejection> {
  let current = load(db, run)?;
  let RunStatus::Paused(PauseReason::LimitReached(RunLimit::PlanLimit { execution })) = *current.status() else {
    return Err(Rejection::RunNotPaused);
  };
  super::idle::resume(db, root, run).map(|prepared| prepared.trying_at_plan_limit(execution))
}
