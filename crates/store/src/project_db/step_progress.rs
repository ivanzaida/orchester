//! The progress line of each step execution (`orchester_core::run::StepProgress`): the latest report replaces the
//! one before.

use orchester_core::{id::StepExecutionId, run::StepProgress, time::Timestamp};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};

use super::{ProjectDb, step_records::StepRecord};
use crate::{StoreError, rows::corrupt};

/// The columns of a `step_progress` row, as a record of the runner carries them (`super::step_records`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct ProgressRow {
  execution: StepExecutionId,
  text: String,
  at: Timestamp,
}

impl ProgressRow {
  pub(super) fn of(execution: StepExecutionId, progress: &StepProgress, at: Timestamp) -> Self {
    Self {
      execution,
      text: progress.as_str().to_owned(),
      at,
    }
  }

  /// Keeps the row's line in place of the one before.
  pub(super) fn save(&self, connection: &Connection) -> Result<(), StoreError> {
    connection.execute(
      "INSERT INTO step_progress (step_execution_id, text, reported_at) VALUES (?1, ?2, ?3)
       ON CONFLICT (step_execution_id) DO UPDATE SET text = excluded.text, reported_at = excluded.reported_at",
      params![self.execution.to_string(), self.text, self.at.unix_millis()],
    )?;
    Ok(())
  }
}

impl ProjectDb {
  /// Keeps `progress` as the line of `execution`, reported at `at`, in place of the one before.
  pub fn report_step_progress(
    &mut self,
    execution: StepExecutionId,
    progress: &StepProgress,
    at: Timestamp,
  ) -> Result<(), StoreError> {
    self.write_step(&StepRecord::progress(execution, progress, at))
  }

  /// The latest line `execution` reported, and when; `None` when it reported none.
  pub fn step_progress(&self, execution: StepExecutionId) -> Result<Option<(StepProgress, Timestamp)>, StoreError> {
    let row = self
      .connection
      .query_row(
        "SELECT text, reported_at FROM step_progress WHERE step_execution_id = ?1",
        [execution.to_string()],
        |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
      )
      .optional()?;
    row
      .map(|(text, at)| {
        let progress = StepProgress::new(&text).map_err(|error| corrupt("step progress", error))?;
        Ok((progress, Timestamp::from_unix_millis(at)))
      })
      .transpose()
  }
}
