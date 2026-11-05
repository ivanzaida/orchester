//! The runs stopped because a resource they pin was archived on the team's server (`archive_stops`).

use orchester_core::{
  id::{RunId, TaskId},
  name::Name,
  time::Timestamp,
};
use rusqlite::params;

use super::ProjectDb;
use crate::{
  StoreError,
  rows::{corrupt, parse},
};

/// A run stopped because the team's resource `resource` (its name then) was archived.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArchiveStop {
  pub run: RunId,
  pub task: TaskId,
  pub resource: Name,
  pub stopped_at: Timestamp,
}

impl ProjectDb {
  /// Records `stop`; a run already recorded keeps its first record.
  pub fn record_archive_stop(&mut self, stop: &ArchiveStop) -> Result<(), StoreError> {
    self.connection.execute(
      "INSERT INTO archive_stops (run_id, task_id, resource_name, stopped_at) VALUES (?1, ?2, ?3, ?4) \
       ON CONFLICT (run_id) DO NOTHING",
      params![
        stop.run.to_string(),
        stop.task.to_string(),
        stop.resource.as_str(),
        stop.stopped_at.unix_millis()
      ],
    )?;
    Ok(())
  }

  /// Every run stopped because a resource it pins was archived, oldest first.
  pub fn archive_stops(&self) -> Result<Vec<ArchiveStop>, StoreError> {
    let mut statement = self
      .connection
      .prepare("SELECT run_id, task_id, resource_name, stopped_at FROM archive_stops ORDER BY stopped_at, run_id")?;
    let rows = statement
      .query_map([], |row| {
        Ok((
          row.get::<_, String>(0)?,
          row.get::<_, String>(1)?,
          row.get::<_, String>(2)?,
          row.get::<_, i64>(3)?,
        ))
      })?
      .collect::<Result<Vec<_>, _>>()?;
    rows
      .into_iter()
      .map(|(run, task, resource, stopped_at)| {
        Ok(ArchiveStop {
          run: parse("archive stop run", &run)?,
          task: parse("archive stop task", &task)?,
          resource: Name::new(&resource).map_err(|_| corrupt("archive stop resource", &resource))?,
          stopped_at: Timestamp::from_unix_millis(stopped_at),
        })
      })
      .collect()
  }
}
