//! Project settings › Limits, one row; a project that never changed them uses the defaults.

use orchester_core::{limits::ProjectLimits, store::SettingsStore};
use rusqlite::{OptionalExtension, params};

use super::ProjectDb;
use crate::{
  StoreError,
  rows::{budget_value, corrupt, duration_value, millis_column, tokens_column},
};

impl SettingsStore for ProjectDb {
  type Error = StoreError;

  fn project_limits(&self) -> Result<ProjectLimits, StoreError> {
    let row: Option<(i64, i64, i64, i64)> = self
      .connection
      .query_row(
        "SELECT max_run_budget, default_run_budget, daily_budget, default_time_limit_ms FROM project_limits",
        [],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
      )
      .optional()?;
    let Some((max, default, daily, time_limit)) = row else {
      return Ok(ProjectLimits::DEFAULT);
    };
    ProjectLimits::new(
      budget_value("maximum run budget", max)?,
      budget_value("default run budget", default)?,
      budget_value("daily project budget", daily)?,
      duration_value("default time limit", time_limit)?,
    )
    .map_err(|error| corrupt("project limits", error))
  }

  fn save_project_limits(&mut self, limits: &ProjectLimits) -> Result<(), StoreError> {
    self.connection.execute(
      "INSERT INTO project_limits (id, max_run_budget, default_run_budget, daily_budget, default_time_limit_ms) \
       VALUES (1, ?1, ?2, ?3, ?4) ON CONFLICT (id) DO UPDATE SET max_run_budget = excluded.max_run_budget, \
       default_run_budget = excluded.default_run_budget, daily_budget = excluded.daily_budget, \
       default_time_limit_ms = excluded.default_time_limit_ms",
      params![
        tokens_column("maximum run budget", limits.max_run_budget().get())?,
        tokens_column("default run budget", limits.default_run_budget().get())?,
        tokens_column("daily project budget", limits.daily_budget().get())?,
        millis_column("default time limit", limits.default_time_limit())?,
      ],
    )?;
    Ok(())
  }
}
