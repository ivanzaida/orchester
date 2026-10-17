//! Authorization uses every retained credential reference, rather than the draft-first display view.

use orchester_core::{
  id::SecretId,
  resource::{ResourceKind, Tool},
  secret,
  store::{ConnectionStore, McpServerStore, ResourceStore},
};
use orchester_store::{ProjectDb, StoreError};

pub(super) fn needs_person(db: &ProjectDb, id: SecretId) -> Result<bool, StoreError> {
  if db
    .connections()?
    .iter()
    .any(|connection| connection.key() == Some(id))
    || db.mcp_servers()?.iter().any(|server| server.secrets().contains(&id))
  {
    return Ok(true);
  }
  for resource in db.resources(ResourceKind::Tool)? {
    if db
      .draft::<Tool>(resource.id())?
      .is_some_and(|draft| secret::tool_uses(draft.body(), id))
      || db
        .versions::<Tool>(resource.id())?
        .iter()
        .any(|version| secret::tool_uses(version.body(), id))
    {
      return Ok(true);
    }
  }
  Ok(false)
}
