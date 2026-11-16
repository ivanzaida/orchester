//! A team project's Pipelines hub: the team's pipelines and the archived ones from the team's configuration as this
//! session read it (`docs/design/team-configuration-live.md`), as [`super::hub`] reads a local project's from its
//! database. Nothing of the team's is read from the project database; the tasks that use a pipeline, and what this
//! computer filled in for the team's profiles and tools, are.

use std::sync::Arc;

use orchester_core::{resource::ResourceKind, store::TaskStore};
use orchester_store::{ProjectDb, StoreError};

use super::{
  data::Assignables,
  flow::FlowRead,
  hub::{ArchivedPipeline, HubPipeline, PipelineHub, TeamPipelines},
  ready::undecided_in,
  team_page::published,
};
use crate::{
  readiness::{ProjectReadiness, ReadinessStanding},
  team_config::TeamConfig,
};

/// The hub of the team's pipelines in `config`, each standing as `readiness` says, with the tasks of `db` that use it.
/// A pipeline whose latest version this computer cannot read as a pipeline is left out, as the library leaves it out
/// (which [`crate::team_config::TeamResource::body`] logs).
pub(super) fn hub(
  db: &ProjectDb,
  config: &TeamConfig,
  readiness: &ProjectReadiness,
) -> Result<PipelineHub, StoreError> {
  let mut assignables = Assignables::of_team(db, config)?;
  assignables.standings = readiness
    .resources()
    .iter()
    .map(|found| (found.resource(), found.standing().clone()))
    .collect();
  let tasks = db.tasks()?;
  let mut pipelines = Vec::new();
  for resource in config.resources(ResourceKind::Pipeline) {
    let Some((version, published_at, body)) = published(resource) else {
      continue;
    };
    let standing = readiness.pipeline(resource.id).map(|found| found.standing().clone());
    let undecided = match &standing {
      Some(ReadinessStanding::Undecided(_)) => undecided_in(&body, readiness),
      _ => Vec::new(),
    };
    let uses = |task: &&orchester_core::task::Task| task.details().pipeline == Some(resource.id);
    pipelines.push(HubPipeline {
      id: resource.id,
      name: resource.name.as_str().into(),
      slug: resource.slug.as_str().into(),
      step_count: body.steps().len(),
      version,
      used_by: tasks.iter().filter(uses).count(),
      published_at,
      standing,
      undecided,
      flow: Arc::new(FlowRead::of_body(&body, &assignables)),
      body: Arc::new(body),
    });
  }
  pipelines.sort_by(|a, b| a.slug.cmp(&b.slug));
  let team = TeamPipelines {
    may_change: db.may_change_configuration()?,
  };
  Ok(PipelineHub::of_team(pipelines, archived(config), assignables, team))
}

/// The team's archived pipelines, the latest archived first.
fn archived(config: &TeamConfig) -> Vec<ArchivedPipeline> {
  let mut archived: Vec<ArchivedPipeline> = config
    .archived_of(ResourceKind::Pipeline)
    .map(|resource| ArchivedPipeline {
      id: resource.id,
      name: resource.name.as_str().into(),
      slug: resource.slug.as_str().into(),
      at: resource.updated.at,
      latest: resource
        .latest
        .as_ref()
        .map(|latest| (latest.number, latest.published_at)),
    })
    .collect();
  archived.sort_by(|a, b| b.at.cmp(&a.at).then_with(|| a.name.cmp(&b.name)));
  archived
}
