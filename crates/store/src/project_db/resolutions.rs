//! `run_resolutions`: merges of runs' base branches waiting in their worktrees for a person to resolve them. A
//! resolution begins only for a run that succeeded and was not merged, and while it is recorded the run's
//! continuations ([`super::runs`]) and its merge ([`super::merges`]) are refused, each in its own transaction, so a
//! return of the run and a resolution never both go ahead.

use orchester_core::{id::RunId, run::RunStatus, time::Timestamp};
use rusqlite::{Connection, OptionalExtension, params};

use super::{ProjectDb, runs::stored_status};
use crate::StoreError;

/// How beginning a resolution went.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResolutionStart {
  /// Recorded: the run does not continue and is not merged until it ends.
  Begun,
  /// The run has not ended as a run that succeeded: an agent may work in its worktree, or will.
  NotEnded,
  /// The run's work was merged.
  Merged,
}

impl ProjectDb {
  /// Records that a merge of `run`'s base branch begins in its worktree at `at`, when the run succeeded and was not
  /// merged; a resolution recorded before is begun again.
  pub fn begin_resolution(&mut self, run: RunId, at: Timestamp) -> Result<ResolutionStart, StoreError> {
    // Immediate, as every transaction here: it holds the write lock from its first read.
    let transaction = self.connection.transaction()?;
    let status = stored_status(&transaction, run)?.ok_or(StoreError::RunNotFound(run))?;
    if status != RunStatus::Succeeded {
      return Ok(ResolutionStart::NotEnded);
    }
    if super::merges::merged(&transaction, run)? {
      return Ok(ResolutionStart::Merged);
    }
    transaction.execute(
      "INSERT INTO run_resolutions (run_id, started_at) VALUES (?1, ?2) \
       ON CONFLICT (run_id) DO UPDATE SET started_at = excluded.started_at",
      params![run.to_string(), at.unix_millis()],
    )?;
    transaction.commit()?;
    Ok(ResolutionStart::Begun)
  }

  /// Ends `run`'s resolution: its merge was committed, aborted, or never began.
  pub fn end_resolution(&mut self, run: RunId) -> Result<(), StoreError> {
    self
      .connection
      .execute("DELETE FROM run_resolutions WHERE run_id = ?1", [run.to_string()])?;
    Ok(())
  }

  /// Ends `run`'s resolution when it began before `before`: one a newer start recorded meanwhile stands. Whether it
  /// ended one.
  pub fn end_resolution_before(&mut self, run: RunId, before: Timestamp) -> Result<bool, StoreError> {
    let ended = self.connection.execute(
      "DELETE FROM run_resolutions WHERE run_id = ?1 AND started_at < ?2",
      params![run.to_string(), before.unix_millis()],
    )?;
    Ok(ended > 0)
  }

  /// When `run`'s resolution began, while it is recorded.
  pub fn resolution(&self, run: RunId) -> Result<Option<Timestamp>, StoreError> {
    resolution(&self.connection, run)
  }
}

pub(super) fn resolution(connection: &Connection, run: RunId) -> Result<Option<Timestamp>, StoreError> {
  let started: Option<i64> = connection
    .query_row(
      "SELECT started_at FROM run_resolutions WHERE run_id = ?1",
      [run.to_string()],
      |row| row.get(0),
    )
    .optional()?;
  Ok(started.map(Timestamp::from_unix_millis))
}
