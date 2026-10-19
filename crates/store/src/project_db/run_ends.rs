//! The ends of the project's runs the Orchestrator chat follows (`orchester_core::orchestrator::RunEnd`): since when
//! it follows them, and when a run last entered a state, by the event that put it there, which keeps each end once.

use orchester_core::{id::RunId, run::EventSequence, time::Timestamp};
use rusqlite::{OptionalExtension, params};

use super::{ProjectDb, runs::sequence_value};
use crate::StoreError;

impl ProjectDb {
  /// Since when the chat follows the ends of the project's runs: an end before it is not followed.
  pub fn run_ends_since(&self) -> Result<Timestamp, StoreError> {
    let since = self.connection.query_row(
      "SELECT run_ends_since FROM orchestrator_follow WHERE id = 1",
      [],
      |row| row.get::<_, i64>(0),
    )?;
    Ok(Timestamp::from_unix_millis(since))
  }

  /// The sequence and time of run `run`'s latest event of kind `kind` (`RunEventKind::name`); `None` without one.
  pub fn latest_run_event(&self, run: RunId, kind: &str) -> Result<Option<(EventSequence, Timestamp)>, StoreError> {
    let latest = self
      .connection
      .query_row(
        "SELECT sequence, at FROM run_events WHERE run_id = ?1 AND kind = ?2 ORDER BY sequence DESC LIMIT 1",
        params![run.to_string(), kind],
        |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
      )
      .optional()?;
    latest
      .map(|(sequence, at)| Ok((sequence_value(sequence)?, Timestamp::from_unix_millis(at))))
      .transpose()
  }
}
