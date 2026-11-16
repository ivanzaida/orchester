//! The runs stopped because a resource they pin was archived on the team's server (`archive_stops`).

use orchester_core::{
  id::{ConnectionId, ResourceId, RunId, TaskId},
  name::Name,
  resource::ResourceKind,
  time::Timestamp,
};
use rusqlite::{OptionalExtension, Row, params};

use super::{
  ProjectDb,
  resources::{kind_text, parse_kind},
};
use crate::{
  StoreError,
  rows::{corrupt, parse},
};

const COLUMNS: &str = "run_id, task_id, resource_name, stopped_at, resource_id, resource_kind, archived_by";

/// A run stopped because the team's resource `resource` (its name then) was archived.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArchiveStop {
  pub run: RunId,
  pub task: TaskId,
  pub resource: Name,
  pub stopped_at: Timestamp,
  /// What was archived and by whom; `None` for a stop recorded before the confirmation named them.
  pub archived: Option<Archived>,
}

/// The resource an archive stopped a run for, and who archived it (the team server's name for them).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Archived {
  pub id: ResourceId,
  pub kind: ResourceKind,
  /// The server's name for who archived it; `None` when only a snapshot told, which does not say.
  pub by: Option<String>,
}

impl ProjectDb {
  /// Records `stop`; a run already recorded keeps its first record.
  pub fn record_archive_stop(&mut self, stop: &ArchiveStop) -> Result<(), StoreError> {
    let archived = stop.archived.as_ref();
    self.connection.execute(
      &format!(
        "INSERT INTO archive_stops ({COLUMNS}) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7) ON CONFLICT (run_id) DO NOTHING"
      ),
      params![
        stop.run.to_string(),
        stop.task.to_string(),
        stop.resource.as_str(),
        stop.stopped_at.unix_millis(),
        archived.map(|archived| archived.id.to_string()),
        archived.map(|archived| kind_text(archived.kind)),
        archived.and_then(|archived| archived.by.as_deref()),
      ],
    )?;
    Ok(())
  }

  /// Every run stopped because a resource it pins was archived, oldest first.
  pub fn archive_stops(&self) -> Result<Vec<ArchiveStop>, StoreError> {
    let mut statement = self.connection.prepare(&format!(
      "SELECT {COLUMNS} FROM archive_stops ORDER BY stopped_at, run_id"
    ))?;
    let rows = statement.query_map([], raw)?.collect::<Result<Vec<_>, _>>()?;
    rows.into_iter().map(stop_of).collect()
  }

  /// The runs the archive of the resource `resource` stopped, oldest first.
  pub fn archive_stops_of_resource(&self, resource: ResourceId) -> Result<Vec<ArchiveStop>, StoreError> {
    self.stops_where("WHERE resource_id = ?1", &resource.to_string())
  }

  /// Notes that the archive of the local MCP server `server` stopped `run`, which [`ProjectDb::record_archive_stop`]
  /// recorded: a local MCP server is no resource, so the stop names it here.
  pub fn record_mcp_server_stop(&mut self, run: RunId, server: ConnectionId) -> Result<(), StoreError> {
    self.connection.execute(
      "INSERT INTO mcp_server_stops (run_id, server_id) VALUES (?1, ?2) ON CONFLICT (run_id) DO NOTHING",
      [run.to_string(), server.to_string()],
    )?;
    Ok(())
  }

  /// The runs the archive of the local MCP server `server` stopped, oldest first.
  pub fn archive_stops_of_mcp_server(&self, server: ConnectionId) -> Result<Vec<ArchiveStop>, StoreError> {
    self.stops_where(
      "WHERE run_id IN (SELECT run_id FROM mcp_server_stops WHERE server_id = ?1)",
      &server.to_string(),
    )
  }

  fn stops_where(&self, clause: &str, value: &str) -> Result<Vec<ArchiveStop>, StoreError> {
    let mut statement = self.connection.prepare(&format!(
      "SELECT {COLUMNS} FROM archive_stops {clause} ORDER BY stopped_at, run_id"
    ))?;
    let rows = statement.query_map([value], raw)?.collect::<Result<Vec<_>, _>>()?;
    rows.into_iter().map(stop_of).collect()
  }

  /// The archive that stopped `run`; `None` when it was not stopped by one.
  pub fn archive_stop_of(&self, run: RunId) -> Result<Option<ArchiveStop>, StoreError> {
    self
      .connection
      .query_row(
        &format!("SELECT {COLUMNS} FROM archive_stops WHERE run_id = ?1"),
        [run.to_string()],
        raw,
      )
      .optional()?
      .map(stop_of)
      .transpose()
  }
}

/// A row as stored: the three that name the archive are all there or none.
type Stored = (
  String,
  String,
  String,
  i64,
  Option<String>,
  Option<String>,
  Option<String>,
);

fn raw(row: &Row<'_>) -> rusqlite::Result<Stored> {
  Ok((
    row.get(0)?,
    row.get(1)?,
    row.get(2)?,
    row.get(3)?,
    row.get(4)?,
    row.get(5)?,
    row.get(6)?,
  ))
}

fn stop_of((run, task, resource, stopped_at, id, kind, by): Stored) -> Result<ArchiveStop, StoreError> {
  let archived = match (id, kind, by) {
    (Some(id), Some(kind), by) => Some(Archived {
      id: parse("archive stop resource id", &id)?,
      kind: parse_kind(&kind)?,
      by,
    }),
    (None, None, None) => None,
    _ => return Err(corrupt("archive stop", &run)),
  };
  Ok(ArchiveStop {
    run: parse("archive stop run", &run)?,
    task: parse("archive stop task", &task)?,
    resource: Name::new(&resource).map_err(|_| corrupt("archive stop resource", &resource))?,
    stopped_at: Timestamp::from_unix_millis(stopped_at),
    archived,
  })
}
