//! Archiving what a local project has and restoring it as it was (ORC-842, decision of 2026-11-08). An archived
//! resource keeps its row, slug and versions, so the runs that pinned it still read them and a pipeline step still
//! names it, but no list or picker offers it. A profile, instruction, skill or tool is archived in `removed_resources`,
//! as a team's archive is followed; a local MCP server, which is a connection with no versions, in
//! `archived_mcp_servers`. Nothing here refuses a resource for being used: what stops, and what is not ready
//! afterwards, is the caller's.

use orchester_core::{
  id::{ConnectionId, ResourceId},
  resource::{Resource, ResourceKind},
  time::Timestamp,
};
use rusqlite::params;

use super::{
  ProjectDb,
  removals::removed_at,
  resources::{kind_text, load_resources},
};
use crate::{StoreError, rows::parse};

impl ProjectDb {
  /// Archives the local resource `id` as of `now`. `false` when it is archived already; a resource the team shares is
  /// the team's to archive and is [`StoreError::ResourceNotFound`] here.
  pub fn archive_local_resource(&mut self, id: ResourceId, now: Timestamp) -> Result<bool, StoreError> {
    self.local_resource(id)?;
    let changed = self.connection.execute(
      "INSERT INTO removed_resources (resource_id, removed_at) VALUES (?1, ?2) ON CONFLICT (resource_id) DO NOTHING",
      params![id.to_string(), now.unix_millis()],
    )?;
    Ok(changed > 0)
  }

  /// Brings the archived local resource `id` back, as it was. `false` when it was not archived.
  pub fn restore_local_resource(&mut self, id: ResourceId) -> Result<bool, StoreError> {
    self.local_resource(id)?;
    let changed = self
      .connection
      .execute("DELETE FROM removed_resources WHERE resource_id = ?1", [id.to_string()])?;
    Ok(changed > 0)
  }

  /// The archived local resources of `kind`, by slug, each with when it was archived.
  pub fn archived_local_resources(&self, kind: ResourceKind) -> Result<Vec<(Resource, Timestamp)>, StoreError> {
    load_resources(
      &self.connection,
      "WHERE kind = ?1 AND sharing IS NOT 'shared' AND id IN (SELECT resource_id FROM removed_resources) ORDER BY slug",
      [kind_text(kind)],
    )?
    .into_iter()
    .map(|resource| {
      let at = removed_at(&self.connection, resource.id())?.ok_or(StoreError::ResourceNotFound(resource.id()))?;
      Ok((resource, at))
    })
    .collect()
  }

  /// Archives the local MCP server `id` as of `now`. `false` when it is archived already.
  pub fn archive_mcp_server(&mut self, id: ConnectionId, now: Timestamp) -> Result<bool, StoreError> {
    let exists: bool = self.connection.query_row(
      "SELECT EXISTS (SELECT 1 FROM mcp_servers WHERE id = ?1)",
      [id.to_string()],
      |row| row.get(0),
    )?;
    if !exists {
      return Err(StoreError::ConnectionNotFound(id));
    }
    let changed = self.connection.execute(
      "INSERT INTO archived_mcp_servers (server_id, archived_at) VALUES (?1, ?2) ON CONFLICT (server_id) DO NOTHING",
      params![id.to_string(), now.unix_millis()],
    )?;
    Ok(changed > 0)
  }

  /// Brings the archived local MCP server `id` back, as it was. `false` when it was not archived.
  pub fn restore_mcp_server(&mut self, id: ConnectionId) -> Result<bool, StoreError> {
    let changed = self.connection.execute(
      "DELETE FROM archived_mcp_servers WHERE server_id = ?1",
      [id.to_string()],
    )?;
    Ok(changed > 0)
  }

  /// The archived local MCP servers, each with when it was archived, oldest first.
  pub fn archived_mcp_servers(&self) -> Result<Vec<(ConnectionId, Timestamp)>, StoreError> {
    let mut statement = self
      .connection
      .prepare("SELECT server_id, archived_at FROM archived_mcp_servers ORDER BY archived_at, server_id")?;
    let rows = statement
      .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)))?
      .collect::<Result<Vec<_>, _>>()?;
    rows
      .into_iter()
      .map(|(id, at)| {
        Ok((
          parse("an archived MCP server's id", &id)?,
          Timestamp::from_unix_millis(at),
        ))
      })
      .collect()
  }

  /// The resource `id` when this project keeps it as its own (not the team's).
  fn local_resource(&self, id: ResourceId) -> Result<Resource, StoreError> {
    load_resources(
      &self.connection,
      "WHERE id = ?1 AND sharing IS NOT 'shared'",
      [id.to_string()],
    )?
    .pop()
    .ok_or(StoreError::ResourceNotFound(id))
  }
}
