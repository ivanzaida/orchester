//! Removing a resource (ORC-107): an agent profile, instruction, skill or tool leaves the project's lists and is
//! no longer pinned by new runs. Its published versions stay, as every resource's do, so runs that already pinned it
//! keep showing and using the version they started with.
//!
//! Pipeline steps name the profile or tool they run by id. While a step of a pipeline's latest version runs it,
//! removing it is refused with those steps.

use super::{
  lifecycle::{ResourceKind, Version, VersionNumber},
  pipeline::{Pipeline, StepKey},
};
use crate::{id::ResourceId, name::Name, store::ResourceStore};

/// A step of a pipeline's latest version that runs the profile or tool being removed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StepUser {
  pub pipeline: ResourceId,
  pub pipeline_name: Name,
  /// The pipeline's latest version, the one whose step it is.
  pub version: VersionNumber,
  /// The step's place in its pipeline, from 1, as the pipeline editor numbers it.
  pub number: usize,
  pub step: StepKey,
  pub step_name: Name,
}

/// What asking to remove a resource did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Removal {
  /// It is removed.
  Removed,
  /// Steps still run it, so nothing changed.
  InUse(Vec<StepUser>),
}

/// The steps of `latest`, the latest version of one pipeline, that run `executor`.
pub fn pipeline_step_users(
  (pipeline, name): (ResourceId, &Name),
  latest: Option<&Version<Pipeline>>,
  executor: ResourceId,
) -> Vec<StepUser> {
  let Some(latest) = latest else {
    return Vec::new();
  };
  latest
    .body()
    .steps_run_by(executor)
    .map(|(place, step)| StepUser {
      pipeline,
      pipeline_name: name.clone(),
      version: latest.number(),
      number: place + 1,
      step: step.key.clone(),
      step_name: step.name.clone(),
    })
    .collect()
}

/// Every step of the project's pipelines that runs `executor`, as a removal would find them now.
pub fn step_users<S: ResourceStore>(store: &S, executor: ResourceId) -> Result<Vec<StepUser>, S::Error> {
  let mut users = Vec::new();
  for pipeline in store.resources(ResourceKind::Pipeline)? {
    let latest = store.latest_version::<Pipeline>(pipeline.id())?;
    users.extend(pipeline_step_users(
      (pipeline.id(), pipeline.name()),
      latest.as_ref(),
      executor,
    ));
  }
  Ok(users)
}
