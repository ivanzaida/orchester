//! The sources listed on the project and on each task (`orchester_core::source`), each list replaced as a whole. A
//! path is kept once per list, in the order given.

use orchester_core::{id::TaskId, source::SourcePath};
use rusqlite::{Connection, OptionalExtension, params};

use super::ProjectDb;
use crate::{StoreError, rows::corrupt};

impl ProjectDb {
  /// The sources every run of the project may read.
  pub fn project_sources(&self) -> Result<Vec<SourcePath>, StoreError> {
    read(
      &self.connection,
      "SELECT path FROM project_sources ORDER BY position",
      params![],
    )
  }

  /// Makes `sources` the project's sources; runs that already started keep theirs.
  pub fn set_project_sources(&mut self, sources: &[SourcePath]) -> Result<(), StoreError> {
    let transaction = self.connection.transaction()?;
    transaction.execute("DELETE FROM project_sources", [])?;
    {
      let mut insert = transaction.prepare("INSERT INTO project_sources (position, path) VALUES (?1, ?2)")?;
      for (position, path) in (0_i64..).zip(once(sources)) {
        insert.execute(params![position, path.as_str()])?;
      }
    }
    transaction.commit()?;
    Ok(())
  }

  /// The sources the runs of `task` may read besides the project's.
  pub fn task_sources(&self, task: TaskId) -> Result<Vec<SourcePath>, StoreError> {
    read(
      &self.connection,
      "SELECT path FROM task_sources WHERE task_id = ?1 ORDER BY position",
      params![task.to_string()],
    )
  }

  /// Makes `sources` the sources of `task`; its runs that already started keep theirs.
  pub fn set_task_sources(&mut self, task: TaskId, sources: &[SourcePath]) -> Result<(), StoreError> {
    let transaction = self.connection.transaction()?;
    let known = transaction
      .query_row("SELECT 1 FROM tasks WHERE id = ?1", [task.to_string()], |_| Ok(()))
      .optional()?;
    if known.is_none() {
      return Err(StoreError::TaskNotFound(task));
    }
    transaction.execute("DELETE FROM task_sources WHERE task_id = ?1", [task.to_string()])?;
    {
      let mut insert = transaction.prepare("INSERT INTO task_sources (task_id, position, path) VALUES (?1, ?2, ?3)")?;
      for (position, path) in (0_i64..).zip(once(sources)) {
        insert.execute(params![task.to_string(), position, path.as_str()])?;
      }
    }
    transaction.commit()?;
    Ok(())
  }
}

/// `sources` with each path once, in order.
fn once(sources: &[SourcePath]) -> Vec<&SourcePath> {
  let mut kept: Vec<&SourcePath> = Vec::new();
  for source in sources {
    if !kept.contains(&source) {
      kept.push(source);
    }
  }
  kept
}

fn read(
  connection: &Connection,
  sql: &str,
  parameters: &[&dyn rusqlite::ToSql],
) -> Result<Vec<SourcePath>, StoreError> {
  let mut statement = connection.prepare(sql)?;
  let paths = statement
    .query_map(parameters, |row| row.get::<_, String>(0))?
    .collect::<Result<Vec<_>, _>>()?;
  paths
    .iter()
    .map(|path| SourcePath::new(path).map_err(|error| corrupt("source", error)))
    .collect()
}
