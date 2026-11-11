//! A team pipeline's change goes to the team server at once and is kept here only once the server took it (D-043): its
//! version in the team's form, and the version and resource as the server answered them, with the steps this computer
//! sent as the local body. Nothing of a pipeline waits in the outbox.

use orchester_api::{SharedResource, SharedVersion, VersionContent};
use orchester_core::{
  id::ResourceId,
  resource::Pipeline,
  sharing::LocalEntities,
  store::{ConnectionStore, McpServerStore, SecretStore},
  time::Timestamp,
};
use orchester_store::{CachedVersion, ProjectDb, StoreError};

use super::{reference, resource, team_body};
use crate::sharing::{SharingError, project::convert};

/// `body`, the steps of the team pipeline `resource`, as the team server takes them. Fails with
/// [`SharingError::PipelineNamesNonTeam`] when a step runs a profile or tool that is not the team's, and with
/// [`SharingError::LocalEntitiesNamed`] when it names what only this computer has; nothing is sent then.
pub(crate) fn pipeline_content(
  db: &ProjectDb,
  resource: ResourceId,
  body: &Pipeline,
) -> Result<VersionContent, SharingError> {
  team_body::check_pipeline(db, resource, body, &team_body::team_executors(db)?)?;
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

/// Keeps `version`, which the team server took of a pipeline with `body` as its steps, and `created`, the resource the
/// server made when it was the pipeline's first. A version this computer published never waits for *Use v<N> here*.
pub(crate) fn keep_pipeline_version(
  db: &mut ProjectDb,
  created: Option<&SharedResource>,
  version: &SharedVersion,
  body: &Pipeline,
) -> Result<(), SharingError> {
  if let Some(shared) = created {
    keep_resource(db, shared)?;
  }
  let kept = CachedVersion {
    number: version.number,
    published_at: Timestamp::from_unix_millis(version.published.at),
    publisher: convert::person(&version.published.by),
    server_body: version.body.clone(),
    hash: version.hash.clone(),
    references: version.references.iter().map(reference).collect(),
    local_body: body.clone(),
    waits_here: false,
  };
  db.cache_shared_version(version.resource, &kept)?;
  Ok(())
}

/// Keeps `shared` as the team server answered it: its name and its revision there.
pub(crate) fn keep_resource(db: &mut ProjectDb, shared: &SharedResource) -> Result<(), SharingError> {
  db.keep_shared_resource(&resource(shared)?, shared.revision)?;
  Ok(())
}
