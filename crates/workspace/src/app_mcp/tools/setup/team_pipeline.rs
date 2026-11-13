//! `share_pipeline`: an Admin of a team project makes one of their own local pipelines, hidden there
//! (`docs/architecture/server.md` row 80), a team pipeline. Its newest version is copied as a new team pipeline's first
//! version on the team server at once ([`crate::sharing::Servers::create_team_pipeline`]), with what its body names as
//! references by name and role, never a secret's value, and kept here only once the server made it; a refusal says why
//! and stores nothing (D-043). The agent profiles and tools its steps run, and its Deliver step's integrator, must be
//! the team's already: a pipeline that names one of this computer's own is refused, naming it, since the team's copy
//! would name what the server does not have. Every member's agents may then run it, so it is a person's decision unless
//! a person lets automation change agent permissions. The local pipeline stays as it is, hidden, its slug shared with
//! the copy while the project is the team's.

use orchester_core::{
  resource::{Pipeline, Resource, ResourceKind, TeamPipelineError},
  store::ResourceStore,
};
use orchester_store::{ProjectDb, ResourceSharing, StoreError};
use serde_json::{Value, json};

use super::{pipelines::InPipeline, widening};
use crate::{
  app_mcp::{
    error::{Missing, ToolError},
    services::{Services, blocking},
  },
  messages::Message,
  sharing::sync::pipeline_executors,
  tasks::outside::Outside,
  team_config::TeamConfig,
};

pub async fn share_pipeline(services: &Services, args: InPipeline) -> Result<Value, ToolError> {
  let shared = services.clone();
  let (copy, id) = blocking(move || {
    let open = shared.project(&args.project)?;
    let db = ProjectDb::open(&open.repo_root)?;
    if db.sharing()?.is_none() {
      return Err(ToolError::refused(
        "team_project_required",
        vec![Message::new("appMcp.errors.sharePipelineLocal")],
      ));
    }
    if !db.may_change_configuration()? {
      return Err(StoreError::NotAdmin.into());
    }
    let local = own_pipeline(&db, &args.pipeline)?;
    let version = db
      .latest_version::<Pipeline>(local.id())?
      .ok_or(StoreError::NotPublished(local.id()))?;
    let config = shared.workspace.team_read(&open.repo_root).loaded()?;
    refuse_local_executors(&db, &config, version.body())?;
    let widens = Message::new("appMcp.widens.teamPipeline").with("pipeline", local.name().as_str().to_owned());
    widening::allowed(&shared, vec![widens])?;
    let id = shared
      .servers
      .create_team_pipeline(
        &open.repo_root,
        (local.slug().clone(), local.name().clone()),
        version.body(),
      )
      .map_err(|refused| ToolError::refused(refused.code(), vec![refused.message()]))?;
    shared.changes.record(open.project.id(), Outside::Pipelines);
    shared.changes.record(open.project.id(), Outside::Tasks);
    Ok((local, id))
  })
  .await?;
  Ok(json!({
    "pipeline": {
      "id": id.to_string(),
      "name": copy.name().as_str(),
      "slug": copy.slug().as_str(),
      "state": "team",
    }
  }))
}

/// The local pipeline `named` by its id or, ignoring case, its name: a pipeline of this computer's own, not the team's.
fn own_pipeline(db: &ProjectDb, named: &str) -> Result<Resource, ToolError> {
  let lowered = named.to_lowercase();
  let mut own = Vec::new();
  for resource in db.kept_resources(ResourceKind::Pipeline)? {
    if !matches!(db.resource_sharing(resource.id())?, Some(ResourceSharing::Shared(_))) {
      own.push(resource);
    }
  }
  let by_id = own.iter().position(|resource| resource.id().to_string() == named);
  by_id
    .or_else(|| {
      own
        .iter()
        .position(|resource| resource.name().as_str().to_lowercase() == lowered)
    })
    .map(|index| own.swap_remove(index))
    .ok_or_else(|| ToolError::not_found(Missing::Pipeline, named))
}

/// Refused, naming the first, when `pipeline` runs an agent profile or tool, or merges with an integrator, that is not
/// the team's: one of `config`, the team's configuration read this session ([`pipeline_executors`]).
fn refuse_local_executors(db: &ProjectDb, config: &TeamConfig, pipeline: &Pipeline) -> Result<(), ToolError> {
  let named = match pipeline.check_team(&pipeline_executors(config)) {
    Ok(()) => return Ok(()),
    Err(
      TeamPipelineError::Profile { profile: id, .. }
      | TeamPipelineError::Tool { tool: id, .. }
      | TeamPipelineError::Integrator { profile: id },
    ) => id,
  };
  let name = match db.resource(named)? {
    Some(found) => found.name().as_str().to_owned(),
    None => config.name(named).map_or_else(|| named.to_string(), str::to_owned),
  };
  Err(ToolError::refused(
    "team_resources_required",
    vec![Message::new("appMcp.errors.sharePipelineLocalExecutors").with("resource", name)],
  ))
}
