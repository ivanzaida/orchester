//! The agent a task's run would start with, before it has a run: the role of the first agent step of its
//! pipeline, in the version it was queued with, else the latest published one. A message written before the
//! run starts reaches that agent, and the composer names it (`YIbm7`: *Explorer · not started*).

use std::sync::Arc;

use orchester_core::{
  resource::{AgentProfile, Pipeline, StepKind},
  store::ResourceStore,
  task::Task,
};
use orchester_store::{ProjectDb, StoreError};

/// The role of the first agent step of `task`'s pipeline; `None` without a pipeline, a published version,
/// an agent step or its profile.
pub(super) fn first_agent(db: &ProjectDb, task: &Task) -> Result<Option<Arc<str>>, StoreError> {
  let Some(pipeline) = task.details().pipeline else {
    return Ok(None);
  };
  let version = match task.pipeline_version() {
    Some(number) => db.version::<Pipeline>(pipeline, number)?,
    None => db.latest_version::<Pipeline>(pipeline)?,
  };
  let Some(version) = version else {
    return Ok(None);
  };
  let profile = version.body().steps().iter().find_map(|step| match step.kind {
    StepKind::Agent { profile, .. } => Some(profile),
    StepKind::Tool { .. } => None,
  });
  let Some(Some(profile)) = profile else {
    return Ok(None);
  };
  let role = db
    .latest_version::<AgentProfile>(profile)?
    .map(|profile| Arc::from(profile.body().role().as_str()));
  Ok(role)
}
