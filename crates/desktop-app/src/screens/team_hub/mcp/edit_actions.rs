//! The presses of an admin's edit of a team MCP server: *Publish for the team*, *Discard draft*, *Cancel sending* and
//! the cancel of a resource that waits to be created. Each is a future action of the panel's [`super::edit::Edit`];
//! each tells the screens that read the connections when it ended.

use std::{
  path::{Path, PathBuf},
  sync::Arc,
};

use lurq::app::ctx::{Ctx, FutureAction};
use orchester_core::{
  id::{ProjectId, ResourceId, VersionId},
  resource::ResourceKind,
};
use orchester_workspace::{
  agents::{
    save_lane::SaveLane,
    team_mcp_edit::{self, EditError},
  },
  app_mcp::setup::NewTeamMcpServer,
  background,
  messages::{self, Message},
  sharing::team_edit::{EditRefused, Edited},
  tasks::outside::Outside,
};

use super::edit::{Discard, Publish};
use crate::services::Services;

type Target = (PathBuf, ProjectId, ResourceId);

/// *Publish for the team*: the fields as typed go to the draft through the lane, which no older save writes after, and
/// the draft is sent. A refused version in the outbox goes back to being the draft first.
pub(super) fn publish(ctx: &mut Ctx, services: &Services, target: &Target, lane: &Arc<SaveLane>) -> Publish {
  let (services, (root, project, id), lane) = (services.clone(), target.clone(), lane.clone());
  ctx.future_action(
    move |(fields, decided, refused): (NewTeamMcpServer, bool, Option<VersionId>)| {
      let (services, root, lane) = (services.clone(), root.clone(), lane.clone());
      async move {
        if !decided {
          return Err(EditRefused::person_only());
        }
        if let Some(version) = refused {
          services
            .servers
            .reopen_draft(&root, ResourceKind::McpServer, version)
            .await
            .map_err(EditRefused::Failed)?;
        }
        let kept = root.clone();
        background::run(move || lane.run_newest(|| team_mcp_edit::save(&kept, id, &fields)))
          .await
          .map_err(EditRefused::Failed)?
          .map_err(|error| EditRefused::Failed(failed(error)))?;
        let settled: Result<Edited, EditRefused> = services.servers.publish_mcp_edit(&root, id).await;
        services.changes.record(project, Outside::Connections);
        settled
      }
    },
  )
}

/// *Discard draft*: through the lane too, so an autosave that started before the press cannot write the draft back.
pub(super) fn discard(ctx: &mut Ctx, services: &Services, target: &Target, lane: &Arc<SaveLane>) -> Discard {
  let (services, (root, project, id), lane) = (services.clone(), target.clone(), lane.clone());
  ctx.future_action(move |(published, refused): (NewTeamMcpServer, Option<VersionId>)| {
    let (services, root, lane) = (services.clone(), root.clone(), lane.clone());
    async move {
      let result = discard_draft(&services, &root, id, (published, refused), lane).await;
      services.changes.record(project, Outside::Connections);
      result
    }
  })
}

/// *Cancel sending*: the version waiting in the outbox goes back to being the draft.
pub(super) fn cancel(ctx: &mut Ctx, services: &Services, target: &Target) -> FutureAction<VersionId, (), Message> {
  let (services, (root, project, _)) = (services.clone(), target.clone());
  ctx.future_action(move |version: VersionId| {
    let (services, root) = (services.clone(), root.clone());
    async move {
      let result = services
        .servers
        .reopen_draft(&root, ResourceKind::McpServer, version)
        .await;
      services.changes.record(project, Outside::Connections);
      result
    }
  })
}

/// *Cancel sending* of a new resource: it is taken back as if never created.
pub(super) fn cancel_created(ctx: &mut Ctx, services: &Services, target: &Target) -> FutureAction<(), (), Message> {
  let (services, (root, project, id)) = (services.clone(), target.clone());
  ctx.future_action(move |(): ()| {
    let (services, root) = (services.clone(), root.clone());
    async move {
      let result = services.servers.cancel_created(&root, id).await;
      services.changes.record(project, Outside::Connections);
      result
    }
  })
}

/// Throws the draft of `id` away, after the refused version `refused` in the outbox went back to being it; the fields
/// the form shows again are the version's own: `published`, or, once a refused version is gone, the team's.
async fn discard_draft(
  services: &Services,
  root: &Path,
  id: ResourceId,
  (published, refused): (NewTeamMcpServer, Option<VersionId>),
  lane: Arc<SaveLane>,
) -> Result<NewTeamMcpServer, Message> {
  if let Some(version) = refused {
    services
      .servers
      .reopen_draft(root, ResourceKind::McpServer, version)
      .await?;
  }
  let kept = root.to_owned();
  background::run(move || lane.run_newest(|| team_mcp_edit::discard(&kept, id)))
    .await?
    .map_err(failed)?;
  if refused.is_none() {
    return Ok(published);
  }
  let kept = root.to_owned();
  background::run(move || team_mcp_edit::published(&kept, id))
    .await?
    .map_err(failed)?
    .ok_or_else(messages::interrupted)
}

/// What a draft that could not be read or saved says.
pub(super) fn failed(error: EditError) -> Message {
  match error {
    EditError::Store(message) => message,
    EditError::Invalid(invalid) => invalid.message(),
  }
}
