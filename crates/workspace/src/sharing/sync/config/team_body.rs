//! The rule of `docs/design/team-configuration.md` for what this computer sends the team: a body refers to other team
//! resources by server id, to secrets by name and to a CLI kind, and holds no connection, MCP server or secret id of
//! this computer. An agent profile is turned into the team's ([`profile`]); a pipeline names its profiles and tools by
//! the id they have on the server, which a team resource keeps on every computer, so its body needs no rewriting and
//! [`check_pipeline`] refuses one that names an id the team does not have, as the team's configuration was read this
//! session ([`TeamConfig`]). A tool names its secret by name ([`tool`]). The bodies of the other kinds are not in the
//! team's form yet, so one that names an entity of this computer is refused here, before anything is sent: Share lists
//! it with the other refusals ([`SharingError::ShareRefused`]) until its kind's body has a team form.

use orchester_api::ReferenceTarget;
use orchester_core::{
  id::{ResourceId, SecretId},
  resource::{
    AgentProfile, Pipeline, ResourceKind, TeamAgentProfile, TeamCounterparts, TeamExecutors, TeamPipelineError,
    TeamProfileError, TeamReviewer, TeamTool, TeamToolError, Tool,
  },
  secret::Secret,
  sharing::{LocalEntities, named},
  store::{ConnectionStore, McpServerStore, ResourceStore, SecretStore},
};
use orchester_store::{ProjectDb, StoreError};
use serde_json::Value;

use crate::{
  sharing::{SharingError, team_edit::Publication},
  team_config::TeamConfig,
};

/// `body` of `resource`, of a kind whose body is not in the team's form yet, as it is: it must name nothing of
/// `entities` and sends no references. Fails with [`SharingError::LocalEntitiesNamed`] when it names a connection, MCP
/// server or secret of `entities`.
pub(crate) fn unchanged(
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
pub(crate) fn reviewer(reviewer: &TeamReviewer) -> Result<Publication, SharingError> {
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
/// ([`ReferenceTarget::of_profile`]). The team MCP servers it may choose are those of `config`. Fails with
/// [`SharingError::LocalEntitiesNamed`] naming a local MCP server that is not a team resource, and with
/// [`StoreError::ConnectionNotFound`] for a connection that is not an agent connection of this computer; nothing is
/// sent.
pub(crate) fn profile(
  db: &ProjectDb,
  resource: ResourceId,
  local: &AgentProfile,
  config: &TeamConfig,
) -> Result<Publication, SharingError> {
  let servers: Vec<ResourceId> = config
    .resources(ResourceKind::McpServer)
    .iter()
    .map(|server| server.id)
    .collect();
  profile_naming(db, resource, local, &servers)
}

/// [`profile`], with `team_mcp_servers` as the team MCP servers a profile may choose: Share's, which are not resources
/// of the project yet.
pub(crate) fn profile_naming(
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

/// `local` in the team's form, from what this computer knows of its connections and MCP servers, with
/// `team_mcp_servers` as the team MCP servers the profile may choose; the outer error is the store failing, the inner
/// one the profile that cannot be published. Publishing sends this.
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

/// The first profile or tool that `pipeline` runs, or integrator it merges with, that is not one of `team`'s.
fn non_team(pipeline: &Pipeline, team: &TeamExecutors) -> Option<ResourceId> {
  match pipeline.check_team(team) {
    Ok(()) => None,
    Err(
      TeamPipelineError::Profile { profile: id, .. }
      | TeamPipelineError::Tool { tool: id, .. }
      | TeamPipelineError::Integrator { profile: id },
    ) => Some(id),
  }
}

/// Fails with [`SharingError::PipelineNamesNonTeam`] when `pipeline`, which Share uploads, runs a profile or tool, or
/// merges with an integrator, that is not one of `team`'s, named as `db` names it.
pub(super) fn check_pipeline(
  db: &ProjectDb,
  resource: ResourceId,
  pipeline: &Pipeline,
  team: &TeamExecutors,
) -> Result<(), SharingError> {
  let Some(named) = non_team(pipeline, team) else {
    return Ok(());
  };
  let name = db
    .resource(named)?
    .map_or_else(|| named.to_string(), |found| found.name().as_str().to_owned());
  Err(SharingError::PipelineNamesNonTeam { resource, name })
}

/// `local`, a version of the tool `resource`, as the team holds it: the secret it sends or connects with, held by id
/// here, becomes the name it has among `secrets`, the project's, and the references repeat that name
/// ([`ReferenceTarget::of_tool`]). A command tool holds none and goes as it is. Fails with
/// [`SharingError::ToolSecretGone`] for a secret the project does not have; nothing is sent.
pub(crate) fn tool(resource: ResourceId, local: &Tool, secrets: &[Secret]) -> Result<Publication, SharingError> {
  let name = |id: SecretId| {
    secrets
      .iter()
      .find(|secret| secret.id() == id)
      .map(|secret| secret.name().clone())
  };
  let team = TeamTool::from_local(local, name).map_err(|error| match error {
    TeamToolError::SecretNotHere => SharingError::ToolSecretGone { resource },
    other => StoreError::Corrupt {
      what: "tool",
      detail: other.to_string(),
    }
    .into(),
  })?;
  Ok(team_tool(&team)?)
}

/// `team`, a version of a team tool already in the team's form, which names its secret by name: it is sent as it is,
/// with the references its body names ([`ReferenceTarget::of_tool`]).
pub(crate) fn team_tool(team: &TeamTool) -> Result<Publication, StoreError> {
  Ok(Publication {
    body: serde_json::to_value(team).map_err(|error| StoreError::Corrupt {
      what: "resource body",
      detail: error.to_string(),
    })?,
    references: ReferenceTarget::of_tool(team),
  })
}

/// The project's secrets a tool's versions may name, archived ones too: a secret is archived, never deleted, so an
/// older version still goes to the team by the name its secret had.
pub(crate) fn named_secrets(db: &ProjectDb) -> Result<Vec<Secret>, StoreError> {
  let mut secrets = db.secrets()?;
  secrets.extend(db.archived_secrets()?);
  Ok(secrets)
}
