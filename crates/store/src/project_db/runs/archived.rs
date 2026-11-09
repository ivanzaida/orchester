//! A run does not start on what a local project archived: the check is made in the transaction that records the run,
//! after the archive's own transaction, so a run is either recorded before an archive (and the archive finds it and
//! stops it) or refused after it. The window's and the runner's preflight say why in words before; this is the last
//! word (`docs/design/team-configuration.md`, decision 11).

use orchester_core::{
  id::{ConnectionId, ResourceId},
  resource::AgentProfile,
  run::{RunSnapshot, StepExecutor},
};
use rusqlite::Connection;

use crate::{StoreError, error::RunStoreError, project_db::resources::load_versions, rows::parse};

/// Refuses a run whose `snapshot` pins an archived local resource, or whose pinned profiles choose an archived local
/// MCP server, naming it.
pub(super) fn refuse(connection: &Connection, snapshot: &RunSnapshot) -> Result<(), RunStoreError> {
  let resources = archived_ids::<ResourceId>(
    connection,
    "SELECT resource_id FROM removed_resources WHERE resource_id IN (SELECT id FROM resources WHERE sharing IS NOT 'shared')",
  )?;
  if let Some(id) = resources.into_iter().find(|id| snapshot.pins(*id)) {
    let name: String = connection.query_row("SELECT name FROM resources WHERE id = ?1", [id.to_string()], |row| {
      row.get(0)
    })?;
    return Err(RunStoreError::UsesArchived(name));
  }
  let servers = archived_ids::<ConnectionId>(connection, "SELECT server_id FROM archived_mcp_servers")?;
  if servers.is_empty() {
    return Ok(());
  }
  for step in snapshot.steps() {
    let StepExecutor::Agent { profile, .. } = step.executor() else {
      continue;
    };
    let versions = load_versions::<AgentProfile>(connection, "WHERE id = ?1", [profile.version.to_string()])?;
    let chosen = versions.first().map(|version| version.body().mcp_servers().ids());
    if let Some(server) = servers
      .iter()
      .find(|server| chosen.is_some_and(|chosen| chosen.contains(server)))
    {
      let name: String = connection.query_row(
        "SELECT name FROM mcp_servers WHERE id = ?1",
        [server.to_string()],
        |row| row.get(0),
      )?;
      return Err(RunStoreError::UsesArchived(name));
    }
  }
  Ok(())
}

fn archived_ids<T: std::str::FromStr>(connection: &Connection, query: &str) -> Result<Vec<T>, StoreError>
where
  T::Err: std::fmt::Display,
{
  let mut statement = connection.prepare(query)?;
  let rows = statement
    .query_map([], |row| row.get::<_, String>(0))?
    .collect::<Result<Vec<_>, _>>()?;
  rows.iter().map(|id| parse("an archived id", id)).collect()
}
