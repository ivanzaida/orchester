//! The progress line of each step execution (`orchester_core::run::StepProgress`): the latest report replaces the
//! one before.

use orchester_core::{id::StepExecutionId, run::StepProgress, time::Timestamp};
use rusqlite::{OptionalExtension, params};

use super::ProjectDb;
use crate::{StoreError, rows::corrupt};

impl ProjectDb {
  /// Keeps `progress` as the line of `execution`, reported at `at`, in place of the one before.
  pub fn report_step_progress(
    &mut self,
    execution: StepExecutionId,
    progress: &StepProgress,
    at: Timestamp,
  ) -> Result<(), StoreError> {
    self.connection.execute(
      "INSERT INTO step_progress (step_execution_id, text, reported_at) VALUES (?1, ?2, ?3)
       ON CONFLICT (step_execution_id) DO UPDATE SET text = excluded.text, reported_at = excluded.reported_at",
      params![execution.to_string(), progress.as_str(), at.unix_millis()],
    )?;
    Ok(())
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
