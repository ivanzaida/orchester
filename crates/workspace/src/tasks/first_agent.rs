//! The agents a task's next run would work with, before it has one: the agent steps of its pipeline, in the
//! version it was queued with, else the latest published one. A message written before the run starts reaches
//! the first of them, and the composer names it (`YIbm7`: *Explorer · not started*); the files it carries must
//! suit every agent it may reach.

use std::sync::Arc;

use orchester_core::{
  connection::AgentProvider,
  conversation::Recipient,
  id::ResourceId,
  resource::{AgentProfile, Pipeline, StepKind},
  store::ResourceStore,
  task::Task,
};
use orchester_store::{ProjectDb, StoreError};

use super::step_executors::reached;

/// The role of the first agent step of `task`'s pipeline; `None` without a pipeline, a published version,
/// an agent step or its profile.
pub(super) fn first_agent(db: &ProjectDb, task: &Task) -> Result<Option<Arc<str>>, StoreError> {
  let Some(Some(profile)) = agent_profiles(db, task)?.into_iter().next() else {
    return Ok(None);
  };
  let role = db
    .latest_version::<AgentProfile>(profile)?
    .map(|profile| Arc::from(profile.body().role().as_str()));
  Ok(role)
}

/// The connections of the agents of `task`'s next run a message to `recipient` may reach: every agent step's for
/// Orchestrator, else the steps of the recipient's profile; none without a pipeline or a published version.
pub(crate) fn next_run_providers(
  db: &ProjectDb,
  task: &Task,
  recipient: Recipient,
) -> Result<Vec<AgentProvider>, StoreError> {
  let mut providers = Vec::new();
  for profile in agent_profiles(db, task)?.into_iter().flatten() {
    if matches!(recipient, Recipient::Agent(to) if to != profile) {
      continue;
    }
    let Some(version) = db.latest_version::<AgentProfile>(profile)? else {
      continue;
    };
    if let Some(provider) = reached(db, version.body().connection())?.provider
      && !providers.contains(&provider)
    {
      providers.push(provider);
    }
  }
  Ok(providers)
}

/// The profile of each agent step of `task`'s pipeline, in order; `None` for a step without one.
fn agent_profiles(db: &ProjectDb, task: &Task) -> Result<Vec<Option<ResourceId>>, StoreError> {
  let Some(pipeline) = task.details().pipeline else {
    return Ok(Vec::new());
  };
  let version = match task.pipeline_version() {
    Some(number) => db.version::<Pipeline>(pipeline, number)?,
    None => db.latest_version::<Pipeline>(pipeline)?,
  };
  let Some(version) = version else {
    return Ok(Vec::new());
  };
  Ok(
    version
      .body()
      .steps()
      .iter()
      .filter_map(|step| match step.kind {
        StepKind::Agent { profile, .. } => Some(profile),
        StepKind::Tool { .. } => None,
      })
      .collect(),
  )
}
