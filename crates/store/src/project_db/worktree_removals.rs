//! `run_worktree_removals`: when runs' own worktrees were removed, one row per run.

use orchester_core::{
  id::{RunId, TaskId},
  integration::RemovalStart,
  store::WorktreeStore,
  time::Timestamp,
};
use rusqlite::{Connection, OptionalExtension, params};

use super::{ProjectDb, runs::stored_status};
use crate::{StoreError, rows::parse};

impl WorktreeStore for ProjectDb {
  type Error = StoreError;

  fn save_worktree_removal(&mut self, run: RunId, at: Timestamp) -> Result<(), StoreError> {
    self.connection.execute(
      "INSERT INTO run_worktree_removals (run_id, removed_at) VALUES (?1, ?2) ON CONFLICT (run_id) DO NOTHING",
      params![run.to_string(), at.unix_millis()],
    )?;
    Ok(())
  }

  fn begin_worktree_removal(&mut self, run: RunId, at: Timestamp) -> Result<RemovalStart, StoreError> {
    // Immediate, as every transaction here: it holds the write lock from its first read.
    let transaction = self.connection.transaction()?;
    let status = stored_status(&transaction, run)?.ok_or(StoreError::RunNotFound(run))?;
    if status.has_live_work() {
      return Ok(RemovalStart::LiveWork);
    }
    if super::local_integration::reserves_worktree(&transaction, run)? {
      return Ok(RemovalStart::LocalIntegration);
    }
    let start = if removal(&transaction, run)?.is_some() {
      RemovalStart::AlreadyRecorded
    } else {
      transaction.execute(
        "INSERT INTO run_worktree_removals (run_id, removed_at) VALUES (?1, ?2)",
        params![run.to_string(), at.unix_millis()],
      )?;
      RemovalStart::Recorded
    };
    transaction.commit()?;
    Ok(start)
  }

  fn undo_worktree_removal(&mut self, run: RunId, at: Timestamp) -> Result<(), StoreError> {
    self.connection.execute(
      "DELETE FROM run_worktree_removals WHERE run_id = ?1 AND removed_at = ?2",
      params![run.to_string(), at.unix_millis()],
    )?;
    Ok(())
  }

  fn worktree_removal(&self, run: RunId) -> Result<Option<Timestamp>, StoreError> {
    removal(&self.connection, run)
  }

  fn worktree_removals(&self, task: TaskId) -> Result<Vec<(RunId, Timestamp)>, StoreError> {
    let mut statement = self.connection.prepare(
      "SELECT runs.id, run_worktree_removals.removed_at FROM run_worktree_removals \
       JOIN runs ON runs.id = run_worktree_removals.run_id WHERE runs.task_id = ?1 ORDER BY runs.number",
    )?;
    let rows = statement.query_map([task.to_string()], |row| {
      Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
    })?;
    rows
      .map(|row| {
        let (run, removed_at) = row?;
        Ok((parse("run id", &run)?, Timestamp::from_unix_millis(removed_at)))
      })
      .collect()
  }
}

/// When the worktree of `run` was removed, as `connection` reads it.
pub(super) fn removal(connection: &Connection, run: RunId) -> Result<Option<Timestamp>, StoreError> {
  let removed: Option<i64> = connection
    .query_row(
      "SELECT removed_at FROM run_worktree_removals WHERE run_id = ?1",
      [run.to_string()],
      |row| row.get(0),
    )
    .optional()?;
  Ok(removed.map(Timestamp::from_unix_millis))
}
