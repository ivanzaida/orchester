//! The rule of `docs/design/team-configuration.md` for what this computer sends the team: a body refers to other team
//! resources by server id and to secrets by name, and holds no connection, MCP server or secret id of this computer.
//! A body that does is refused here, before anything is sent. Turning such a body into one that follows the rule is
//! the profile and tool bodies' work (ORC-820, ORC-830); until then it stays on this computer.
//!
//! A pipeline names its profiles and tools by the id they have on the server, which a team resource keeps on every
//! computer, so its body needs no rewriting; [`check_pipeline`] refuses one that names an id the team does not have.

use std::collections::HashSet;

use orchester_core::{
  id::ResourceId,
  resource::{Pipeline, ResourceKind, TeamExecutors, TeamPipelineError},
  sharing::{LocalEntities, named},
  store::ResourceStore,
};
use orchester_store::{ProjectDb, PublishState, ResourceSharing, StoreError};
use serde_json::Value;

use crate::sharing::SharingError;

/// Fails with [`SharingError::LocalEntitiesNamed`] when `body` of `resource` names a connection, MCP server or secret
/// of `entities`.
pub(super) fn check(resource: ResourceId, body: &Value, entities: LocalEntities<'_>) -> Result<(), SharingError> {
  let names: Vec<String> = named(body, entities)
    .iter()
    .map(|reference| reference.name().to_owned())
    .collect();
  if names.is_empty() {
    Ok(())
  } else {
    Err(SharingError::LocalEntitiesNamed { resource, names })
  }
}

/// The agent profiles and tools of the team in the project: those the server has (marked shared here) and the
/// Admin's new ones that wait for the server ([`pending_team`]), not a resource only this computer keeps.
pub(crate) fn team_executors(db: &ProjectDb) -> Result<TeamExecutors, StoreError> {
  let pending = pending_team(db)?;
  let ids = |kind| -> Result<Vec<ResourceId>, StoreError> {
    let mut team = Vec::new();
    for resource in db.kept_resources(kind)? {
      let shared = matches!(db.resource_sharing(resource.id())?, Some(ResourceSharing::Shared(_)));
      if shared || pending.contains(&resource.id()) {
        team.push(resource.id());
      }
    }
    Ok(team)
  };
  Ok(TeamExecutors::new(
    ids(ResourceKind::AgentProfile)?,
    ids(ResourceKind::Tool)?,
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

/// The first profile or tool that `pipeline` runs, or integrator it merges with, that is not one of `team`'s, by its
/// name when this computer has the resource and by its id otherwise.
pub(crate) fn non_team_name(
  db: &ProjectDb,
  pipeline: &Pipeline,
  team: &TeamExecutors,
) -> Result<Option<String>, StoreError> {
  let named = match pipeline.check_team(team) {
    Ok(()) => return Ok(None),
    Err(
      TeamPipelineError::Profile { profile: id, .. }
      | TeamPipelineError::Tool { tool: id, .. }
      | TeamPipelineError::Integrator { profile: id },
    ) => id,
  };
  Ok(Some(db.resource(named)?.map_or_else(
    || named.to_string(),
    |found| found.name().as_str().to_owned(),
  )))
}

/// Fails with [`SharingError::PipelineNamesNonTeam`] when `pipeline` runs a profile or tool, or merges with an
/// integrator, that is not one of `team`'s ([`non_team_name`]).
pub(super) fn check_pipeline(
  db: &ProjectDb,
  resource: ResourceId,
  pipeline: &Pipeline,
  team: &TeamExecutors,
) -> Result<(), SharingError> {
  match non_team_name(db, pipeline, team)? {
    Some(name) => Err(SharingError::PipelineNamesNonTeam { resource, name }),
    None => Ok(()),
  }
}
