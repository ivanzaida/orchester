//! `share_pipeline`: an Admin of a team project makes one of their own local pipelines, hidden there
//! (`docs/architecture/server.md` row 80), a team pipeline. Its newest version is copied as a new team pipeline's first
//! version and sent through the shared configuration's publishing ([`crate::sharing::Servers::send_published`]), with
//! what its body names as references by name and role, never a secret's value. The agent profiles and tools its steps
//! run must be the team's already: a pipeline whose steps run one of this computer's own is refused, naming them, since
//! the team's copy would name what the server does not have. Every member's agents may then run it, so it is a
//! person's decision unless a person lets automation change agent permissions. The local pipeline stays as it is,
//! hidden, but for its slug, which it gives way to the team's.

use std::collections::HashSet;

use orchester_core::{
  id::ResourceId,
  resource::{Pipeline, Resource, ResourceKind},
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::{ProjectDb, PublishState, ResourceSharing, StoreError};
use serde_json::{Value, json};

use super::{pipelines::InPipeline, widening};
use crate::{
  app_mcp::{
    error::{Missing, ToolError},
    services::{Services, blocking},
  },
  messages::Message,
  tasks::outside::Outside,
};

pub async fn share_pipeline(services: &Services, args: InPipeline) -> Result<Value, ToolError> {
  let shared = services.clone();
  let (root, copy) = blocking(move || {
    let open = shared.project(&args.project)?;
    let mut db = ProjectDb::open(&open.repo_root)?;
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
    refuse_local_executors(&db, version.body())?;
    let widens = Message::new("appMcp.widens.teamPipeline").with("pipeline", local.name().as_str().to_owned());
    widening::allowed(&shared, vec![widens])?;
    let copy = db.copy_for_the_team::<Pipeline>(local.id(), Timestamp::now())?;
    shared.changes.record(open.project.id(), Outside::Pipelines);
    shared.changes.record(open.project.id(), Outside::Tasks);
    Ok((open.repo_root, copy))
  })
  .await?;
  // The copy waits for the server once kept here: a send that fails leaves it waiting, sent again on reconnect.
  let not_sent = services.servers.send_published(&root).await.err();
  let id = copy.id();
  let team = blocking(move || {
    let db = ProjectDb::open(&root)?;
    Ok(matches!(db.resource_sharing(id)?, Some(ResourceSharing::Shared(_))))
  })
  .await?;
  let mut result = json!({
    "pipeline": {
      "id": id.to_string(),
      "name": copy.name().as_str(),
      "slug": copy.slug().as_str(),
      "state": if team { "team" } else { "waiting" },
    }
  });
  if let Some(reason) = not_sent {
    tracing::warn!(key = reason.key, "a pipeline shared for the team waits to be sent");
    result["not_sent"] = json!({ "code": "team_share_waiting" });
  }
  Ok(result)
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

/// Refused, naming them, when steps of `pipeline` run an agent profile or tool that is not the team's.
fn refuse_local_executors(db: &ProjectDb, pipeline: &Pipeline) -> Result<(), ToolError> {
  let waiting = pending_team(db)?;
  let mut local = Vec::new();
  for executor in pipeline.steps().iter().filter_map(|step| step.kind.executor()) {
    let name = match db.resource(executor)? {
      // One this computer no longer has is none of the team's either.
      None => executor.to_string(),
      Some(_) if waiting.contains(&executor) => continue,
      Some(resource) => {
        if matches!(db.resource_sharing(executor)?, Some(ResourceSharing::Shared(_))) {
          continue;
        }
        resource.name().as_str().to_owned()
      }
    };
    if !local.contains(&name) {
      local.push(name);
    }
  }
  if local.is_empty() {
    return Ok(());
  }
  Err(ToolError::refused(
    "team_resources_required",
    vec![Message::new("appMcp.errors.sharePipelineLocalExecutors").with("resources", local.join(", "))],
  ))
}

/// The resources with versions published here that wait for the team server and none it refused: the team's, not
/// there yet. One the server refused (a slug it has, say) never became the team's, so it is this computer's own.
fn pending_team(db: &ProjectDb) -> Result<HashSet<ResourceId>, StoreError> {
  let waiting = db.waiting_publishes()?;
  let refused: HashSet<ResourceId> = waiting
    .iter()
    .filter(|waiting| matches!(waiting.state, PublishState::Refused { .. }))
    .map(|waiting| waiting.resource)
    .collect();
  Ok(
    waiting
      .into_iter()
      .map(|waiting| waiting.resource)
      .filter(|resource| !refused.contains(resource))
      .collect(),
  )
}
