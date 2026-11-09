//! The agents the review page offers to run the returned step with (ORC-888; `Review - v2` 12 `IfAmk`): the project's
//! agent profiles that have a published version, and the profile the run's own step runs with.

use std::sync::Arc;

use orchester_core::{
  id::ResourceId,
  resource::StepKey,
  run::{Run, StepExecutor},
};
use orchester_store::ProjectDb;

use super::ReviewRefusal;
use crate::agents::data::published_profiles;

/// An agent profile as the choice names it: its role and the model it runs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReviewAgent {
  pub id: ResourceId,
  pub name: Arc<str>,
  pub model: Arc<str>,
}

/// The project's agent profiles with a published version, ordered by slug, as their latest versions name them.
pub fn published(db: &ProjectDb) -> Result<Vec<ReviewAgent>, ReviewRefusal> {
  Ok(
    published_profiles(db)?
      .into_iter()
      .map(|entry| ReviewAgent {
        id: entry.id,
        name: entry.profile.role().as_str().into(),
        model: entry.profile.model().as_str().into(),
      })
      .collect(),
  )
}

/// The profile and model the pipeline gives `run`'s step `key`, as the run pinned them; `None` for a tool or test step.
pub(super) fn pinned(run: &Run, key: &StepKey) -> Option<(ResourceId, Arc<str>)> {
  run
    .snapshot()
    .steps()
    .iter()
    .find(|step| step.key() == key)
    .and_then(|step| match step.executor() {
      StepExecutor::Agent { profile, model, .. } => Some((profile.resource, model.as_str().into())),
      StepExecutor::Tool { .. } | StepExecutor::Test => None,
    })
}
