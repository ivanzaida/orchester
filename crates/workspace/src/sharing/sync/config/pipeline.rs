//! A team pipeline's change goes to the team server at once (D-043): its version in the team's form. Nothing of it is
//! kept in the project's database (`docs/design/team-configuration-live.md`); the team's configuration in memory takes
//! what the server answered ([`crate::sharing::project::team_pipeline`]).

use orchester_api::VersionContent;
use orchester_core::{
  id::ResourceId,
  resource::{Pipeline, ResourceKind, TeamExecutors},
  sharing::LocalEntities,
  store::{ConnectionStore, McpServerStore, SecretStore},
};
use orchester_store::{ProjectDb, StoreError};

use super::team_body;
use crate::{sharing::SharingError, team_config::TeamConfig};

/// `body`, the steps of the team pipeline `resource`, as the team server takes them. Fails with
/// [`SharingError::PipelineNamesNonTeam`] when a step runs a profile or tool that is not one of `team`'s, and with
/// [`SharingError::LocalEntitiesNamed`] when it names what only this computer has; nothing is sent then.
pub(crate) fn pipeline_content(
  db: &ProjectDb,
  team: &TeamExecutors,
  resource: ResourceId,
  body: &Pipeline,
) -> Result<VersionContent, SharingError> {
  team_body::check_pipeline(db, resource, body, team)?;
  let value = serde_json::to_value(body).map_err(|error| StoreError::Corrupt {
    what: "resource body",
    detail: error.to_string(),
  })?;
  let (connections, mcp_servers, secrets) = (db.connections()?, db.mcp_servers()?, db.secrets()?);
  let entities = LocalEntities {
    connections: &connections,
    mcp_servers: &mcp_servers,
    secrets: &secrets,
  };
  let publication = team_body::unchanged(resource, value, entities)?;
  Ok(VersionContent {
    body: publication.body,
    references: publication.references,
  })
}

/// The team's agent profiles and tools a team pipeline may run: those of `config`, the team's configuration read this
/// session. Nothing of the team is read from the project's database.
pub(crate) fn pipeline_executors(config: &TeamConfig) -> TeamExecutors {
  let ids = |kind| config.resources(kind).into_iter().map(|resource| resource.id);
  TeamExecutors::new(ids(ResourceKind::AgentProfile), ids(ResourceKind::Tool))
}
