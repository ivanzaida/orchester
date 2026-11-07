//! The rule of `docs/design/team-configuration.md` for what this computer sends the team: a body refers to other team
//! resources by server id and to secrets by name, and holds no connection, MCP server or secret id of this computer.
//! A body that does is refused here, before anything is sent. Turning such a body into one that follows the rule is
//! the profile and tool bodies' work (ORC-820, ORC-830); until then it stays on this computer.

use orchester_core::{
  id::ResourceId,
  sharing::{LocalEntities, named},
};
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
