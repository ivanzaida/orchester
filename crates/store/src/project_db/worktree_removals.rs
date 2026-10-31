//! `run_worktree_removals`: when runs' own worktrees were removed, one row per run; `run_worktrees_not_removed`: the
//! folder a removal that could not finish left, and why (ORC-418).

use std::{collections::HashMap, path::Path};

use orchester_core::{
  command::CommandRunner,
  id::{RunId, TaskId},
  integration::{NotRemoved, RemovalStart, sweep},
  store::WorktreeStore,
  time::Timestamp,
};
use rusqlite::{Connection, OptionalExtension, params};

use super::{ProjectDb, runs::stored_status};
use crate::{StoreError, rows::parse};

impl ProjectDb {
  /// The folder the last removal of the worktrees of `run` left on disk, and why, as recorded; whether it is still
  /// there is the file system's to say.
  pub fn worktree_not_removed(&self, run: RunId) -> Result<Option<NotRemoved>, StoreError> {
    Ok(
      self
        .connection
        .query_row(
          "SELECT path, reason FROM run_worktrees_not_removed WHERE run_id = ?1",
          [run.to_string()],
          |row| {
            Ok(NotRemoved {
              path: row.get(0)?,
              reason: row.get(1)?,
            })
          },
        )
        .optional()?,
    )
  }

  /// The names of the worktree folders runs own, `.orchester/worktrees/<name>` — each run's id and its step
  /// executions' ids — with whether the run ended: it succeeded, failed or was cancelled, or it was interrupted and its
  /// worktree removal recorded, so no agent of it works there again.
  pub fn worktree_folder_owners(&self) -> Result<HashMap<String, bool>, StoreError> {
    const ENDED: &str = "(runs.status IN ('succeeded', 'failed', 'cancelled') \
                         OR (runs.status = 'interrupted' AND removals.run_id IS NOT NULL))";
    let sql = format!(
      "SELECT runs.id, {ENDED} FROM runs \
       LEFT JOIN run_worktree_removals AS removals ON removals.run_id = runs.id \
       UNION ALL \
       SELECT executions.id, {ENDED} FROM step_executions AS executions \
       JOIN attempts ON attempts.id = executions.attempt_id JOIN runs ON runs.id = attempts.run_id \
       LEFT JOIN run_worktree_removals AS removals ON removals.run_id = runs.id"
    );
    let mut statement = self.connection.prepare(&sql)?;
    let rows = statement.query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, bool>(1)?)))?;
    Ok(rows.collect::<Result<_, _>>()?)
  }

  /// Sweeps the worktree folders of the repository at `repo` ([`sweep`]) that no git worktree, as the `git` program
  /// run by `runner` lists them, and no run here that has not ended owns, each deletion logged. It only frees space:
  /// what cannot be read or deleted is logged, and the next sweep tries again. The runner sweeps at its start, and
  /// the runner and the desktop after each removal.
  pub fn sweep_worktree_folders(&self, runner: &impl CommandRunner, (git, repo): (&Path, &Path)) {
    let owners = match self.worktree_folder_owners() {
      Ok(owners) => owners,
      Err(error) => {
        tracing::warn!(%error, "cannot read which runs own the worktree folders; none is swept");
        return;
      }
    };
    let ended = |name: &str| owners.get(name).copied().unwrap_or(false);
    match sweep(runner, (git, repo), ended) {
      Ok(swept) => {
        for folder in swept {
          match folder.outcome {
            Ok(()) => tracing::info!(path = %folder.path.display(), "deleted a worktree folder no run uses"),
            Err(error) => {
              tracing::warn!(path = %folder.path.display(), %error, "cannot delete a worktree folder no run uses");
            }
          }
        }
      }
      Err(error) => tracing::warn!(%error, "cannot sweep the worktree folders"),
    }
  }
}

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
      // A merge waiting in the worktree for the person goes with it: the person removes it.
      transaction.execute("DELETE FROM run_resolutions WHERE run_id = ?1", [run.to_string()])?;
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

  fn save_worktree_not_removed(&mut self, run: RunId, left: &NotRemoved, at: Timestamp) -> Result<(), StoreError> {
    self.connection.execute(
      "INSERT INTO run_worktrees_not_removed (run_id, path, reason, at) VALUES (?1, ?2, ?3, ?4) \
       ON CONFLICT (run_id) DO UPDATE SET path = excluded.path, reason = excluded.reason, at = excluded.at",
      params![run.to_string(), left.path, left.reason, at.unix_millis()],
    )?;
    Ok(())
  }

  fn clear_worktree_not_removed(&mut self, run: RunId) -> Result<(), StoreError> {
    self.connection.execute(
      "DELETE FROM run_worktrees_not_removed WHERE run_id = ?1",
      [run.to_string()],
    )?;
    Ok(())
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
