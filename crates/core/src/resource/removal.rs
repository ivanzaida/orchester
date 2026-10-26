//! Removing a resource (ORC-107): an agent profile, instruction, skill or tool leaves the project's lists and is
//! no longer pinned by new runs. Its published versions stay, as every resource's do, so runs that already pinned it
//! keep showing and using the version they started with.
//!
//! Pipeline steps name the profile or tool they run by id. While a step of a pipeline's latest version or draft
//! runs it, removing it is refused with those steps; *Remove and reset* removes it anyway after giving each such
//! pipeline a draft whose steps no longer run it ([`super::Pipeline::without_executor`]), so the draft asks for
//! another one before it can be published.

use super::{
  lifecycle::{Draft, ResourceKind, Version, VersionNumber},
  pipeline::{Pipeline, Step, StepKey},
};
use crate::{id::ResourceId, name::Name, store::ResourceStore};

/// Where a pipeline step that runs a resource is: the pipeline's latest published version, or its draft.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StepVersion {
  Published(VersionNumber),
  /// The draft, which publishes as this version number.
  Draft(u32),
}

/// A pipeline step that runs the profile or tool being removed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StepUser {
  pub pipeline: ResourceId,
  pub pipeline_name: Name,
  pub version: StepVersion,
  /// The step's place in its pipeline, from 1, as the pipeline editor numbers it.
  pub number: usize,
  pub step: StepKey,
  pub step_name: Name,
}

/// What asking to remove a resource did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Removal {
  /// It is removed; the pipelines that ran it (when reset) have drafts without it.
  Removed,
  /// Steps still run it, so nothing changed.
  InUse(Vec<StepUser>),
}

/// Whether a removal may clear the resource from the steps that run it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RemoveMode {
  /// Remove only a resource no step runs.
  Unused,
  /// Clear it from every step that runs it, then remove it.
  Reset,
}

/// The steps of one pipeline that run `executor`: those of its latest version, then those of its draft the version
/// does not have.
pub fn pipeline_step_users(
  (pipeline, name): (ResourceId, &Name),
  (draft, latest): (Option<&Draft<Pipeline>>, Option<&Version<Pipeline>>),
  executor: ResourceId,
) -> Vec<StepUser> {
  let user = |version, (place, step): (usize, &Step)| StepUser {
    pipeline,
    pipeline_name: name.clone(),
    version,
    number: place + 1,
    step: step.key.clone(),
    step_name: step.name.clone(),
  };
  let mut users: Vec<StepUser> = latest.map_or_else(Vec::new, |latest| {
    latest
      .body()
      .steps_run_by(executor)
      .map(|step| user(StepVersion::Published(latest.number()), step))
      .collect()
  });
  if let Some(draft) = draft {
    let number = latest.map_or(1, |latest| latest.number().get().saturating_add(1));
    let unlisted: Vec<StepUser> = draft
      .body()
      .steps_run_by(executor)
      .filter(|(_, step)| !users.iter().any(|listed| listed.step == step.key))
      .map(|step| user(StepVersion::Draft(number), step))
      .collect();
    users.extend(unlisted);
  }
  users
}

/// Every step of the project's pipelines that runs `executor`, as a removal would find them now.
pub fn step_users<S: ResourceStore>(store: &S, executor: ResourceId) -> Result<Vec<StepUser>, S::Error> {
  let mut users = Vec::new();
  for pipeline in store.resources(ResourceKind::Pipeline)? {
    let draft = store.draft::<Pipeline>(pipeline.id())?;
    let latest = store.latest_version::<Pipeline>(pipeline.id())?;
    users.extend(pipeline_step_users(
      (pipeline.id(), pipeline.name()),
      (draft.as_ref(), latest.as_ref()),
      executor,
    ));
  }
  Ok(users)
}
