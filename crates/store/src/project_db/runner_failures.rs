//! Why the runner could not record or carry on a run it interrupted (ORC-75, `RunnerFailure`), kept with that
//! interruption's event: what the Orchestrator may continue reads it, and so do `get_run`, `list_runs` and the task's
//! Summary. Written only together with the interruption ([`ProjectDb::append_runner_failure`]).

use orchester_core::{
  id::RunId,
  run::{EventSequence, InterruptCause, Run, RunEvent, RunEventKind, RunnerFailure},
  time::Timestamp,
};
use rusqlite::{OptionalExtension, Transaction, params};

use super::{ProjectDb, runs::With};
use crate::{RunStoreError, StoreError, rows::corrupt};

const STORAGE_CONTENTION: &str = "storage_contention";

impl ProjectDb {
  /// `RunStore::append_events` for the runner's interruption of `run` after its own failure: `failure`, why, is kept
  /// with the interruption, the last of `events`, in the same transaction, so whoever reads the interruption reads
  /// why too.
  pub fn append_runner_failure(
    &mut self,
    run: &Run,
    events: &[RunEvent],
    failure: RunnerFailure,
  ) -> Result<(), RunStoreError> {
    let interruption = events.last().filter(|event| {
      matches!(
        event.kind(),
        RunEventKind::Interrupted {
          cause: InterruptCause::RunnerFailed
        }
      )
    });
    let Some(interruption) = interruption else {
      return Err(RunStoreError::EventsMismatch(
        "the runner's failure is kept only with its interruption",
      ));
    };
    self.append(run, events, With::RunnerFailure(interruption.sequence(), failure))
  }
  /// Why the runner failed when it last interrupted `run`; `None` when its last interruption was for another cause,
  /// recorded before the kind was kept, or when it was never interrupted.
  pub fn runner_failure(&self, run: RunId) -> Result<Option<RunnerFailure>, StoreError> {
    let kind: Option<Option<String>> = self
      .connection
      .query_row(
        "SELECT f.kind FROM run_events e
         LEFT JOIN run_runner_failures f ON f.run_id = e.run_id AND f.sequence = e.sequence
         WHERE e.run_id = ?1 AND e.kind = 'interrupted'
         ORDER BY e.sequence DESC LIMIT 1",
        [run.to_string()],
        |row| row.get(0),
      )
      .optional()?;
    kind.flatten().as_deref().map(parse).transpose()
  }

  /// How many runs of the project the runner interrupted because the database stayed busy at `since` or later.
  pub fn contended_runs_since(&self, since: Timestamp) -> Result<usize, StoreError> {
    let count: i64 = self.connection.query_row(
      "SELECT COUNT(DISTINCT f.run_id) FROM run_runner_failures f
       JOIN run_events e ON e.run_id = f.run_id AND e.sequence = f.sequence
       WHERE f.kind = ?1 AND e.at >= ?2",
      params![STORAGE_CONTENTION, since.unix_millis()],
      |row| row.get(0),
    )?;
    usize::try_from(count).map_err(|error| corrupt("contended run count", error))
  }
}

/// Keeps `failure` with the interruption of `run` at `sequence`, in the transaction that records it.
pub(super) fn insert(
  transaction: &Transaction<'_>,
  run: RunId,
  sequence: EventSequence,
  failure: RunnerFailure,
) -> Result<(), StoreError> {
  let sequence = i64::try_from(sequence.get()).map_err(|_| StoreError::Unrepresentable("run event sequence"))?;
  transaction.execute(
    "INSERT INTO run_runner_failures (run_id, sequence, kind) VALUES (?1, ?2, ?3)",
    params![run.to_string(), sequence, text(failure)],
  )?;
  Ok(())
}

const fn text(failure: RunnerFailure) -> &'static str {
  match failure {
    RunnerFailure::StorageContention => STORAGE_CONTENTION,
    RunnerFailure::StorageFailure => "storage_failure",
    RunnerFailure::Other => "other",
  }
}

fn parse(value: &str) -> Result<RunnerFailure, StoreError> {
  match value {
    STORAGE_CONTENTION => Ok(RunnerFailure::StorageContention),
    "storage_failure" => Ok(RunnerFailure::StorageFailure),
    "other" => Ok(RunnerFailure::Other),
    other => Err(corrupt("runner failure", other)),
  }
}
