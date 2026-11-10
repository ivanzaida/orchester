//! `share_pipeline`: an Admin of a team project makes one of their own local pipelines, hidden there
//! (`docs/architecture/server.md` row 80), a team pipeline. Its newest version is created on the team server as a new
//! team pipeline's first version at once (`docs/design/team-configuration-live.md`, rule 6), with what its body names
//! as references by name and role, never a secret's value; while the server cannot be reached nothing is created. The
//! agent profiles and tools its steps run, and its Deliver step's integrator, must be the team's already, as the team's
//! configuration was read this session: a pipeline that names one of this computer's own is refused, naming it, since
//! the team's copy would name what the server does not have. Every member's agents may then run it, so it is a person's
//! decision unless a person lets automation change agent permissions. The local pipeline stays as it is, hidden.

use orchester_core::{
  id::ResourceId,
  resource::{Pipeline, Resource, ResourceKind},
  sharing::LocalEntities,
  store::{ConnectionStore, McpServerStore, ResourceStore, SecretStore},
};
use orchester_store::{ProjectDb, ResourceSharing, StoreError};
use serde_json::{Value, json};

use super::{
  created::{create_team, not_team_form},
  pipelines::InPipeline,
  team_fill_ins::team_config,
  widening,
};
use crate::{
  app_mcp::{
    error::{Missing, ToolError},
    services::{Services, blocking},
  },
  messages::Message,
  sharing::{SharingError, sync::team_body, team_edit::NewTeamResource},
  tasks::outside::Outside,
};

pub async fn share_pipeline(services: &Services, args: InPipeline) -> Result<Value, ToolError> {
  let shared = services.clone();
  let (open, new) = blocking(move || {
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
    let config = team_config(&shared, &open.repo_root)?;
    let local = own_pipeline(&db, &args.pipeline)?;
    let version = db
      .latest_version::<Pipeline>(local.id())?
      .ok_or(StoreError::NotPublished(local.id()))?;
    let id = ResourceId::generate();
    if let Err(error) = team_body::check_team_pipeline(&config, id, version.body()) {
      return Err(local_executors(error));
    }
    let widens = Message::new("appMcp.widens.teamPipeline").with("pipeline", local.name().as_str().to_owned());
    widening::allowed(&shared, vec![widens])?;
    let (connections, mcp_servers, secrets) = (db.connections()?, db.mcp_servers()?, db.secrets()?);
    let entities = LocalEntities {
      connections: &connections,
      mcp_servers: &mcp_servers,
      secrets: &secrets,
    };
    let body = serde_json::to_value(version.body()).map_err(|error| StoreError::Corrupt {
      what: "resource body",
      detail: error.to_string(),
    })?;
    let publication = team_body::unchanged(id, body, entities).map_err(not_team_form)?;
    Ok((
      open,
      NewTeamResource {
        id,
        kind: ResourceKind::Pipeline,
        name: local.name().clone(),
        publication,
      },
    ))
  })
  .await?;
  let name = new.name.as_str().to_owned();
  let id = create_team(services, &open.repo_root, new).await?;
  services.changes.record(open.project.id(), Outside::Pipelines);
  services.changes.record(open.project.id(), Outside::Tasks);
  Ok(json!({ "pipeline": { "id": id.to_string(), "name": name, "state": "team" } }))
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

/// The refusal of a pipeline that runs an agent profile or tool, or merges with an integrator, that is not the team's.
fn local_executors(error: SharingError) -> ToolError {
  match error {
    SharingError::PipelineNamesNonTeam { name, .. } => ToolError::refused(
      "team_resources_required",
      vec![Message::new("appMcp.errors.sharePipelineLocalExecutors").with("resource", name)],
    ),
    error => not_team_form(error),
  }
}
