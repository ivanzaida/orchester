//! A run stopped at its account's plan limit, as the task page offers its choice (`docs/design/agent-accounts.md`,
//! `EwAhH`): the stopped step's connection and the account it ran on, the CLI's last reading (its windows, when the
//! limit resets), and the other connections of that CLI on other accounts the step could continue on now, as a new
//! session for this run only. Reading blocks on SQLite; the UI calls it off the UI thread.

use std::path::Path;

use orchester_core::{
  connection::{AgentConnection, invocation::Platform, plan_limits::LimitReading},
  id::{ConnectionId, RunId},
  resource::AgentProfile,
  run::{PauseReason, Run, RunLimit, RunStatus, StepExecutor},
  store::{ConnectionStore, ResourceStore, RunStore},
};
use orchester_ipc::Command;
use orchester_store::{ProjectDb, StoreError};

use super::detail::run_store_error;

/// What the choice of a run stopped at a plan limit needs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlanStop {
  /// The connection the stopped step ran on.
  pub connection: AgentConnection,
  /// The last report of the run's CLIs' limits: the windows and when the limit resets.
  pub reading: Option<LimitReading>,
  /// The connections of the same CLI on other accounts that can start now.
  pub others: Vec<AgentConnection>,
}

impl PlanStop {
  /// What run `run` of the project at `repo_root` stopped at; `None` when it is not stopped at a plan limit.
  pub fn load_in(repo_root: &Path, run: RunId) -> Result<Option<Self>, StoreError> {
    let db = ProjectDb::open(repo_root)?;
    let Some(run) = db.run(run).map_err(run_store_error)? else {
      return Ok(None);
    };
    let RunStatus::Paused(PauseReason::LimitReached(RunLimit::PlanLimit { execution })) = *run.status() else {
      return Ok(None);
    };
    let Some(step) = run.execution(execution).map(|execution| execution.step().clone()) else {
      return Ok(None);
    };
    let Some(connection) = step_connection(&db, &run, &step)? else {
      return Ok(None);
    };
    let provider = connection.provider();
    let account = connection.account().id(provider);
    let others = db
      .connections()?
      .into_iter()
      .filter_map(|other| AgentConnection::try_from(other).ok())
      .filter(|other| {
        other.provider() == provider
          && other.account().id(provider) != account
          && other.status().is_usable()
          && other
            .account()
            .start_problem(provider, Platform::current(), Path::is_dir)
            .is_none()
      })
      .collect();
    Ok(Some(Self {
      reading: db.last_limit_reading(run.id())?,
      connection,
      others,
    }))
  }
}

/// The connection `step` of `run` runs on: one chosen for this run, else its pinned profile's.
fn step_connection(
  db: &ProjectDb,
  run: &Run,
  step: &orchester_core::resource::StepKey,
) -> Result<Option<AgentConnection>, StoreError> {
  let id = match db.step_connection(run.id(), step)? {
    Some(id) => id,
    None => {
      let Some(StepExecutor::Agent { profile, .. }) = run.snapshot().step(step).map(|step| step.executor()) else {
        return Ok(None);
      };
      let Some(version) = db.version::<AgentProfile>(profile.resource, profile.number)? else {
        return Ok(None);
      };
      version.body().connection()
    }
  };
  Ok(
    db.connection(id)?
      .and_then(|connection| AgentConnection::try_from(connection).ok()),
  )
}

/// The command that continues `run`'s stopped step now on `connection`, another account's.
pub const fn continue_on(run: RunId, connection: ConnectionId) -> Command {
  Command::ContinueOnConnection { run, connection }
}
