//! The rule of `docs/design/team-configuration.md` for what this computer sends the team: a body refers to other team
//! resources by server id, to secrets by name and to a CLI kind, and holds no connection, MCP server or secret id of
//! this computer. An agent profile is turned into the team's ([`profile`]); a pipeline names its profiles and tools by
//! the id they have on the server, which a team resource keeps on every computer, so its body needs no rewriting and
//! [`check_pipeline`] refuses one that names an id the team does not have. The bodies of the other kinds are not in the
//! team's form yet, so one that names an entity of this computer is refused here, before anything is sent: Share lists
//! it with the other refusals ([`SharingError::ShareRefused`]) until its kind's body has a team form.

use std::collections::HashSet;

use orchester_api::ReferenceTarget;
use orchester_core::{
  id::ResourceId,
  resource::{
    AgentProfile, Pipeline, Resource, ResourceKind, TeamAgentProfile, TeamCounterparts, TeamExecutors,
    TeamPipelineError, TeamProfileError, TeamReviewer,
  },
  sharing::{LocalEntities, named},
  store::{ConnectionStore, McpServerStore, ResourceStore},
};
use orchester_store::{ProjectDb, PublishState, ResourceSharing, StoreError};
use serde_json::Value;

use crate::sharing::SharingError;

/// A version's body as the team holds it, and the references that body names.
pub(super) struct Publication {
  pub body: Value,
  pub references: Vec<ReferenceTarget>,
}

/// `body` of `resource`, of a kind whose body is not in the team's form yet, as it is: it must name nothing of
/// `entities` and sends no references. Fails with [`SharingError::LocalEntitiesNamed`] when it names a connection, MCP
/// server or secret of `entities`.
pub(super) fn unchanged(
  resource: ResourceId,
  body: Value,
  entities: LocalEntities<'_>,
) -> Result<Publication, SharingError> {
  let names: Vec<String> = named(&body, entities)
    .iter()
    .map(|reference| reference.name().to_owned())
    .collect();
  if names.is_empty() {
    Ok(Publication {
      body,
      references: Vec::new(),
    })
  } else {
    Err(SharingError::LocalEntitiesNamed { resource, names })
  }
}

/// `reviewer`, a version of a team reviewer, as the team holds it: its body already names a CLI kind and no connection,
/// so it is sent as it is, with the references that body names ([`ReferenceTarget::of_reviewer`]).
pub(super) fn reviewer(reviewer: &TeamReviewer) -> Result<Publication, SharingError> {
  Ok(Publication {
    body: serde_json::to_value(reviewer).map_err(|error| StoreError::Corrupt {
      what: "resource body",
      detail: error.to_string(),
    })?,
    references: ReferenceTarget::of_reviewer(reviewer),
  })
}

/// `local`, a version of the agent profile `resource`, as the team holds it: its connection becomes the CLI kind it
/// runs, its MCP servers the team MCP servers they are, and the references repeat what that body names
/// ([`ReferenceTarget::of_profile`]). Fails with [`SharingError::LocalEntitiesNamed`] naming a local MCP server that
/// is not a team resource, and with [`StoreError::ConnectionNotFound`] for a connection that is not an agent connection
/// of this computer; nothing is sent.
pub(super) fn profile(db: &ProjectDb, resource: ResourceId, local: &AgentProfile) -> Result<Publication, SharingError> {
  profile_naming(db, resource, local, &team_mcp_servers(db)?)
}

/// [`profile`], with `team_mcp_servers` as the team MCP servers a profile may choose: Share's, which are not resources
/// of the project yet.
pub(super) fn profile_naming(
  db: &ProjectDb,
  resource: ResourceId,
  local: &AgentProfile,
  team_mcp_servers: &[ResourceId],
) -> Result<Publication, SharingError> {
  let team = team_form_naming(db, local, team_mcp_servers)?.map_err(|error| refused(resource, error))?;
  Ok(Publication {
    body: serde_json::to_value(&team).map_err(|error| StoreError::Corrupt {
      what: "resource body",
      detail: error.to_string(),
    })?,
    references: ReferenceTarget::of_profile(&team),
  })
}

/// `local` in the team's form, from what this computer knows of its connections and MCP servers; the outer error is the
/// store failing, the inner one the profile that cannot be published. Publishing sends this, and the readiness of a
/// version that waits for the server reads it ([`crate::readiness`]).
pub(crate) fn team_form(
  db: &ProjectDb,
  local: &AgentProfile,
) -> Result<Result<TeamAgentProfile, TeamProfileError>, StoreError> {
  team_form_naming(db, local, &team_mcp_servers(db)?)
}

/// [`team_form`], with `team_mcp_servers` as the team MCP servers the profile may choose.
fn team_form_naming(
  db: &ProjectDb,
  local: &AgentProfile,
  team_mcp_servers: &[ResourceId],
) -> Result<Result<TeamAgentProfile, TeamProfileError>, StoreError> {
  let (connections, local_mcp_servers) = (db.connections()?, db.mcp_servers()?);
  Ok(TeamAgentProfile::from_local(
    local,
    TeamCounterparts {
      connections: &connections,
      local_mcp_servers: &local_mcp_servers,
      team_mcp_servers,
    },
  ))
}

/// The team MCP servers of the project, by server id.
fn team_mcp_servers(db: &ProjectDb) -> Result<Vec<ResourceId>, StoreError> {
  Ok(
    db.resources(ResourceKind::McpServer)?
      .iter()
      .map(Resource::id)
      .collect(),
  )
}

fn refused(resource: ResourceId, error: TeamProfileError) -> SharingError {
  match error {
    TeamProfileError::LocalMcpServer(name) => SharingError::LocalEntitiesNamed {
      resource,
      names: vec![name],
    },
    TeamProfileError::WritableFolders { folders } => SharingError::WritableFoldersStay { resource, folders },
    TeamProfileError::Connection(connection) => StoreError::ConnectionNotFound(connection).into(),
    other @ (TeamProfileError::Duplicate { .. } | TeamProfileError::TooMany { .. }) => StoreError::Corrupt {
      what: "agent profile",
      detail: other.to_string(),
    }
    .into(),
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
