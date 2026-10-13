//! A run stopped at its account's plan limit, as the task page offers its choice (`docs/design/agent-accounts.md`,
//! `EwAhH`): the stopped step's connection and the account it ran on, the CLI's last reading (its windows, when the
//! limit resets), and the other connections of that CLI on other accounts the step could continue on now, as a new
//! session for this run only. Reading blocks on SQLite; the UI calls it off the UI thread.

use std::path::Path;

use orchester_core::{
  connection::{AgentConnection, invocation::Platform, plan_limits::LimitReading},
  id::{AgentAccountId, ConnectionId, RunId},
  run::{PauseReason, RunLimit, RunStatus},
  store::{ActivityStore, ConnectionStore, RunStore},
};
use orchester_ipc::Command;
use orchester_store::{ProjectDb, StoreError};

use super::detail::run_store_error;

/// What the choice of a run stopped at a plan limit needs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlanStop {
  /// The connection the stopped step runs on.
  pub connection: AgentConnection,
  /// The account the stopped step's agent started on, as the runner recorded it: the connection's account may have
  /// changed since.
  pub account: Option<AgentAccountId>,
  /// The last report of that account's limits during the run: the windows and when the limit resets.
  pub reading: Option<LimitReading>,
  /// The connections of the same CLI on other accounts that can start now.
  pub others: Vec<AgentConnection>,
  /// Whether the stopped step's agent started: `false` for a step that waits for its account's reset before its
  /// agent starts (a new run on an account at its limit, `XPjSc`).
  pub started: bool,
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
    let Some(connection) = db.step_agent_connection(&run, &step)? else {
      return Ok(None);
    };
    let provider = connection.provider();
    let account = db
      .execution_account(execution)?
      .or_else(|| connection.account().id(provider));
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
    let started = db
      .invocations(run.id())?
      .iter()
      .any(|invocation| invocation.execution == execution);
    Ok(Some(Self {
      started,
      reading: db.plan_stop_reading(run.id(), execution, connection.account().id(provider))?,
      connection,
      account,
      others,
    }))
  }
}

/// The command that continues `run`'s stopped step now on `connection`, another account's.
pub const fn continue_on(run: RunId, connection: ConnectionId) -> Command {
  Command::ContinueOnConnection { run, connection }
}

/// The command that tries `run`'s stopped step on its own account now, though its limit was last reported reached.
pub const fn try_now(run: RunId) -> Command {
  Command::TryAtPlanLimit { run }
}
