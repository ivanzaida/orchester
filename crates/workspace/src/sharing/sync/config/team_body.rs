//! The rule of `docs/design/team-configuration.md` for what this computer sends the team: a body refers to other team
//! resources by server id, to secrets by name and to a CLI kind, and holds no connection, MCP server or secret id of
//! this computer. An agent profile is turned into the team's ([`profile`]); the bodies of the other kinds are not yet,
//! so one that names an entity of this computer is refused here, before anything is sent, and stays on this computer
//! until its kind's body changes (ORC-830).

use orchester_api::ReferenceTarget;
use orchester_core::{
  id::ResourceId,
  resource::{AgentProfile, ResourceKind, TeamAgentProfile, TeamCounterparts, TeamProfileError},
  sharing::{LocalEntities, named},
  store::{ConnectionStore, McpServerStore, ResourceStore},
};
use orchester_store::{ProjectDb, StoreError};
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

/// `local`, a version of the agent profile `resource`, as the team holds it: its connection becomes the CLI kind it
/// runs, its MCP servers the team MCP servers they are, and the references repeat what that body names
/// ([`ReferenceTarget::of_profile`]). Fails with [`SharingError::LocalEntitiesNamed`] naming a local MCP server that
/// is not a team resource, and with [`StoreError::ConnectionNotFound`] for a connection that is not an agent connection
/// of this computer; nothing is sent.
pub(super) fn profile(db: &ProjectDb, resource: ResourceId, local: &AgentProfile) -> Result<Publication, SharingError> {
  let (connections, local_mcp_servers) = (db.connections()?, db.mcp_servers()?);
  let team_mcp_servers: Vec<ResourceId> = db
    .resources(ResourceKind::McpServer)?
    .iter()
    .map(orchester_core::resource::Resource::id)
    .collect();
  let team = TeamAgentProfile::from_local(
    local,
    TeamCounterparts {
      connections: &connections,
      local_mcp_servers: &local_mcp_servers,
      team_mcp_servers: &team_mcp_servers,
    },
  )
  .map_err(|error| refused(resource, error))?;
  Ok(Publication {
    body: serde_json::to_value(&team).map_err(|error| StoreError::Corrupt {
      what: "resource body",
      detail: error.to_string(),
    })?,
    references: ReferenceTarget::of_profile(&team),
  })
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
