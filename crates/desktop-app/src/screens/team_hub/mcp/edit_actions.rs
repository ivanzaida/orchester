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
  resource::VersionNumber,
};
use orchester_workspace::{
  agents::{
    save_lane::SaveLane,
    team_mcp_conflict::Based,
    team_mcp_edit::{self, EditError},
    team_mcp_merge::{McpField, Pick},
  },
  app_mcp::setup::NewTeamMcpServer,
  background,
  messages::{self, Message},
  sharing::{
    NotRestored,
    team_edit::{EditRefused, Edited},
  },
  tasks::outside::Outside,
};

use super::edit::{Discard, Publish, Rebase, Restore, opened_on};
use crate::services::Services;

type Target = (PathBuf, ProjectId, ResourceId);

/// *Publish for the team*: the fields as typed go to the draft through the lane, which no older save writes after, and
/// the draft is sent. A refused version in the outbox goes back to being the draft first.
pub(super) fn publish(ctx: &mut Ctx, services: &Services, target: &Target, lane: &Arc<SaveLane>) -> Publish {
  let (services, (root, project, id), lane) = (services.clone(), target.clone(), lane.clone());
  ctx.future_action(
    move |(fields, base, decided, refused): (NewTeamMcpServer, VersionNumber, bool, Option<VersionId>)| {
      let (services, root, lane) = (services.clone(), root.clone(), lane.clone());
      async move {
        if !decided {
          return Err(EditRefused::person_only());
        }
        if let Some(version) = refused {
          services
            .servers
            .reopen_mcp_draft(&root, version)
            .await
            .map_err(EditRefused::Failed)?;
        }
        let kept = root.clone();
        background::run(move || lane.run_newest(|| team_mcp_edit::save(&kept, id, &fields, base)))
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
  ctx.future_action(move |refused: Option<VersionId>| {
    let (services, root, lane) = (services.clone(), root.clone(), lane.clone());
    async move {
      let result = discard_draft(&services, &root, id, refused, lane).await;
      services.changes.record(project, Outside::Connections);
      result
    }
  })
}

/// *Cancel sending*: the version waiting in the outbox goes back to being the draft; the form is then edited from the
/// version the team server numbered, which is what a save or a publish names.
pub(super) fn cancel(
  ctx: &mut Ctx,
  services: &Services,
  target: &Target,
) -> FutureAction<VersionId, Option<Based>, Message> {
  let (services, (root, project, id)) = (services.clone(), target.clone());
  ctx.future_action(move |version: VersionId| {
    let (services, root) = (services.clone(), root.clone());
    async move {
      let result = match services.servers.reopen_mcp_draft(&root, version).await {
        Ok(()) => reopened_base(&root, id).await,
        Err(message) => Err(message),
      };
      services.changes.record(project, Outside::Connections);
      result
    }
  })
}

/// What the form is edited from after a version of the outbox went back to being the draft; `None` for a server the
/// form cannot hold.
async fn reopened_base(root: &Path, id: ResourceId) -> Result<Option<Based>, Message> {
  let kept = root.to_owned();
  let edit = background::run(move || team_mcp_edit::read(&kept, id))
    .await?
    .map_err(failed)?;
  Ok(opened_on(edit.as_ref()))
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

/// Throws the draft of `id` away, after the version of the outbox `refused` went back to being it; the form shows the
/// version this computer uses again: its number and fields.
async fn discard_draft(
  services: &Services,
  root: &Path,
  id: ResourceId,
  refused: Option<VersionId>,
  lane: Arc<SaveLane>,
) -> Result<Based, Message> {
  if let Some(version) = refused {
    services.servers.reopen_mcp_draft(root, version).await?;
  }
  let kept = root.to_owned();
  background::run(move || lane.run_newest(|| team_mcp_edit::discard(&kept, id)))
    .await?
    .map_err(failed)?;
  let kept = root.to_owned();
  let edit = background::run(move || team_mcp_edit::read(&kept, id))
    .await?
    .map_err(failed)?
    .ok_or_else(messages::interrupted)?;
  Ok(Based {
    number: edit.number,
    fields: edit.published,
  })
}

/// *Rebase my draft on vN*: the merge of the draft with the team's version, over the version the draft was edited from,
/// with what was picked for each field both changed. Only a person (or automation they let decide) rebases.
pub(super) fn rebase(ctx: &mut Ctx, services: &Services, target: &Target) -> Rebase {
  let (services, (root, project, id)) = (services.clone(), target.clone());
  ctx.future_action(
    move |(outbox, expected, picks, decided): (Option<VersionId>, VersionNumber, Vec<(McpField, Pick)>, bool)| {
      let (services, root) = (services.clone(), root.clone());
      async move {
        if !decided {
          return Err(Message::new("sharing.errors.personOnly"));
        }
        let result = services
          .servers
          .rebase_mcp_draft(&root, id, (outbox, expected), picks)
          .await;
        services.changes.record(project, Outside::Connections);
        result
      }
    },
  )
}

/// *Open vN*: the version of the person's own in the outbox that met the team's goes back to being the draft, which is
/// kept, so the page can show the team's version.
pub(super) fn open_new(
  ctx: &mut Ctx,
  services: &Services,
  target: &Target,
) -> FutureAction<Option<VersionId>, Option<Based>, Message> {
  let (services, (root, project, id)) = (services.clone(), target.clone());
  ctx.future_action(move |outbox: Option<VersionId>| {
    let (services, root) = (services.clone(), root.clone());
    async move {
      let result = match outbox {
        Some(version) => match services.servers.reopen_mcp_draft(&root, version).await {
          Ok(()) => reopened_base(&root, id).await,
          Err(message) => Err(message),
        },
        None => Ok(None),
      };
      services.changes.record(project, Outside::Connections);
      result
    }
  })
}

/// *Restore*: the team's server brings the archived resource back for everyone. Only a person (or automation they let
/// decide) restores, and only an admin: the server says so otherwise.
pub(super) fn restore(ctx: &mut Ctx, services: &Services, target: &Target) -> Restore {
  let (services, (root, project, id)) = (services.clone(), target.clone());
  ctx.future_action(move |decided: bool| {
    let (services, root) = (services.clone(), root.clone());
    async move {
      if !decided {
        return Err(Message::new("sharing.errors.personOnly"));
      }
      let result = services
        .servers
        .restore_resource(&root, id)
        .await
        .map_err(NotRestored::message);
      services.changes.record(project, Outside::Connections);
      result
    }
  })
}

/// What a draft that could not be read or saved says.
pub(super) fn failed(error: EditError) -> Message {
  match error {
    EditError::Store(message) => message,
    EditError::Invalid(invalid) => invalid.message(),
  }
}
