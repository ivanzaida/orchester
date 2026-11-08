//! The runs stopped because a resource they pin was archived on the team's server (`archive_stops`).

use orchester_core::{
  id::{ResourceId, RunId, TaskId},
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
