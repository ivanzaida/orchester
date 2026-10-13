use orchester_core::{
  id::RunId,
  integration::local::{LocalProcess, LocalProcessStore, ProcessIdentity},
};
use rusqlite::{OptionalExtension, params};

use super::super::ProjectDb;
use crate::{StoreError, rows::corrupt};

impl LocalProcessStore for ProjectDb {
  type Error = StoreError;

  fn local_process(&self, run: RunId) -> Result<Option<LocalProcess>, StoreError> {
    let row: Option<(i64, Option<String>, bool)> = self
      .connection
      .query_row(
        "SELECT sequence, identity, finished FROM local_integration_process WHERE run_id = ?1",
        [run.to_string()],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
      )
      .optional()?;
    row
      .map(|(sequence, identity, finished)| {
        let identity = identity
          .map(|json| serde_json::from_str(&json).map_err(|error| corrupt("local process", error)))
          .transpose()?;
        let sequence = u64::try_from(sequence).map_err(|_| corrupt("local process", "negative sequence"))?;
        Ok(LocalProcess {
          sequence,
          identity,
          finished,
        })
      })
      .transpose()
  }

  fn begin_local_process(&mut self, run: RunId) -> Result<u64, StoreError> {
    let transaction = self.connection.transaction()?;
    if super::super::worktree_removals::removal(&transaction, run)?.is_some()
      && super::read_operation(&transaction, run)?
        .is_none_or(|operation| operation.merged().is_none() && operation.preservation().is_none())
    {
      return Err(corrupt(
        "local process",
        "the task worktree was removed without retained recovery",
      ));
    }
    let previous: Option<(i64, bool)> = transaction
      .query_row(
        "SELECT sequence, finished FROM local_integration_process WHERE run_id = ?1",
        [run.to_string()],
        |row| Ok((row.get(0)?, row.get(1)?)),
      )
      .optional()?;
    let unfinished: bool = transaction.query_row(
      "SELECT EXISTS(SELECT 1 FROM local_integration_process WHERE finished = 0)",
      [],
      |row| row.get(0),
    )?;
    if unfinished {
      return Err(corrupt("local process", "an earlier execution has not ended"));
    }
    let sequence = previous
      .map_or(0, |(sequence, _)| sequence)
      .checked_add(1)
      .ok_or(StoreError::Unrepresentable("process sequence"))?;
    transaction.execute(
      "INSERT INTO local_integration_process (run_id, sequence, identity, finished) VALUES (?1, ?2, NULL, 0) \
       ON CONFLICT (run_id) DO UPDATE SET sequence = excluded.sequence, identity = NULL, finished = 0",
      params![run.to_string(), sequence],
    )?;
    transaction.commit()?;
    u64::try_from(sequence).map_err(|_| corrupt("local process", "negative sequence"))
  }

  fn local_process_started(
    &mut self,
    run: RunId,
    sequence: u64,
    identity: Option<&ProcessIdentity>,
  ) -> Result<(), StoreError> {
    let identity = identity
      .map(serde_json::to_string)
      .transpose()
      .map_err(|error| corrupt("local process", error))?;
    let sequence = i64::try_from(sequence).map_err(|_| StoreError::Unrepresentable("process sequence"))?;
    let changed = self.connection.execute(
      "UPDATE local_integration_process SET identity = ?3 WHERE run_id = ?1 AND sequence = ?2 AND finished = 0 AND identity IS NULL",
      params![run.to_string(), sequence, identity],
    )?;
    if changed != 1 {
      return Err(corrupt("local process", "stale process writer"));
    }
    Ok(())
  }

  fn finish_local_process(&mut self, run: RunId, sequence: u64) -> Result<(), StoreError> {
    let sequence = i64::try_from(sequence).map_err(|_| StoreError::Unrepresentable("process sequence"))?;
    let changed = self.connection.execute(
      "UPDATE local_integration_process SET finished = 1 WHERE run_id = ?1 AND sequence = ?2",
      params![run.to_string(), sequence],
    )?;
    if changed != 1 {
      return Err(corrupt("local process", "stale process writer"));
    }
    Ok(())
  }
}
