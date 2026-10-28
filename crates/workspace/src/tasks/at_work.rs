//! Who works on a project's running runs, as the Overview's *At work* names them (`Overview · redesign` `iRvT6`, 01
//! `LXhun`): when each run started, how many steps its pipeline has, and the CLI, account and model its step's agent
//! runs on. Read with the project's tasks ([`super::data`]) for the runs that run only, so the page never reads a run
//! of its own.

use std::{collections::BTreeMap, sync::Arc};

use orchester_core::{
  connection::{AgentProvider, Connection, ConnectionKind},
  id::{AgentAccountId, RunId, TaskId},
  resource::AgentProfile,
  run::{Run, StepExecutor},
  store::{ActivityStore, ConnectionStore, ResourceStore},
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};

use super::list::CurrentStep;

/// What a reading of the tasks keeps for the Overview besides their latest runs.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RunFacts {
  /// Each latest run that runs: when it started, its steps and who works on its step.
  pub at_work: BTreeMap<RunId, RunAtWork>,
  /// When each latest run merged in Orchester was merged.
  pub merged_at: BTreeMap<RunId, Timestamp>,
  /// The task before each task of a reviewed group in the group's order, which the group starts first.
  pub previous_in_group: BTreeMap<TaskId, TaskId>,
}

/// The agent of a running run's step.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StepAgent {
  pub provider: AgentProvider,
  /// Its maker's mark may stand for it: not a CLI on someone else's API.
  pub marked: bool,
  /// The account its latest launch started on, else the one its connection runs on; `None` for an agent without one.
  pub account: Option<AgentAccountId>,
  /// The model its latest launch runs, else its profile's.
  pub model: Arc<str>,
}

/// A running run as *At work* shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunAtWork {
  pub started_at: Timestamp,
  /// How many steps its pipeline has.
  pub steps: usize,
  /// `None` for a tool step, before the first step, or when its profile or connection is gone.
  pub agent: Option<StepAgent>,
}

/// `run`, which runs and stands at `step`.
pub(super) fn read(db: &ProjectDb, run: &Run, step: Option<&CurrentStep>) -> Result<RunAtWork, StoreError> {
  let agent = match step_agent(db, run, step)? {
    None => None,
    Some((profile, connection, provider)) => {
      let launched = db
        .invocations(run.id())?
        .into_iter()
        .filter(|invocation| invocation.model().is_some())
        .max_by_key(|invocation| invocation.started_at);
      let account = db.invocation_accounts(run.id())?.last().map(|(_, account)| *account);
      Some(StepAgent {
        provider,
        marked: connection.api_provider().is_none(),
        account: account.or_else(|| connection.account().id(provider)),
        model: launched
          .and_then(|invocation| invocation.model().map(|model| Arc::from(model.as_str())))
          .unwrap_or_else(|| Arc::from(profile.model().as_str())),
      })
    }
  };
  Ok(RunAtWork {
    started_at: run.created_at(),
    steps: run.snapshot().steps().len(),
    agent,
  })
}

/// The provider of the connection whose agent runs `step` of `run`, as its pinned profile names it; `None` for a tool
/// step, or a profile or connection no longer found.
pub(super) fn provider(
  db: &ProjectDb,
  run: &Run,
  step: Option<&CurrentStep>,
) -> Result<Option<AgentProvider>, StoreError> {
  Ok(step_agent(db, run, step)?.map(|(.., provider)| provider))
}

/// The pinned profile of `step`'s agent, its connection and the connection's provider.
fn step_agent(
  db: &ProjectDb,
  run: &Run,
  step: Option<&CurrentStep>,
) -> Result<Option<(AgentProfile, Connection, AgentProvider)>, StoreError> {
  let Some(StepExecutor::Agent { profile, .. }) = step
    .and_then(|step| run.snapshot().steps().get(step.number.checked_sub(1)?))
    .map(|snapshot| snapshot.executor())
  else {
    return Ok(None);
  };
  let Some(version) = db.version::<AgentProfile>(profile.resource, profile.number)? else {
    return Ok(None);
  };
  let profile = version.body().clone();
  Ok(
    ConnectionStore::connection(db, profile.connection())?.and_then(|connection| match connection.kind() {
      ConnectionKind::Agent(provider) => {
        let provider = *provider;
        Some((profile, connection, provider))
      }
      ConnectionKind::GitHub { .. } => None,
    }),
  )
}
