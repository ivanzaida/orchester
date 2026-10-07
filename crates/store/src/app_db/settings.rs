//! Settings of the application rather than of a project: whether automation may answer waiting tool
//! requests through Orchester's MCP server (off until a person turns it on), and the warning
//! threshold the Usage dashboard draws in Plan limits (limits belong to accounts, so every project
//! shows the same line; 80 % until it is changed).

use orchester_core::usage::limits::WarningThreshold;
use rusqlite::params;

use super::AppDb;
use crate::{StoreError, rows::corrupt};

pub(super) const MIGRATION: &str = r"
CREATE TABLE app_settings (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  automation_answers_approvals INTEGER NOT NULL DEFAULT 0 CHECK (automation_answers_approvals IN (0, 1))
) STRICT;
INSERT INTO app_settings (id) VALUES (1);
";

pub(super) const THRESHOLD_MIGRATION: &str = r"
-- Usage › Plan limits › Warning threshold (ORC-72), a whole percentage of a window.
ALTER TABLE app_settings ADD COLUMN usage_warning_threshold INTEGER NOT NULL DEFAULT 80
  CHECK (usage_warning_threshold BETWEEN 1 AND 100);
";

impl AppDb {
  /// Whether automation may answer waiting tool requests (*Let automation answer approvals*).
  pub fn automation_answers_approvals(&self) -> Result<bool, StoreError> {
    Ok(self.connection.query_row(
      "SELECT automation_answers_approvals FROM app_settings WHERE id = 1",
      [],
      |row| row.get(0),
    )?)
  }

  pub fn set_automation_answers_approvals(&mut self, allowed: bool) -> Result<(), StoreError> {
    self.connection.execute(
      "UPDATE app_settings SET automation_answers_approvals = ?1 WHERE id = 1",
      params![allowed],
    )?;
    Ok(())
  }

  /// Where Plan limits draws its dashed warning line.
  pub fn usage_warning_threshold(&self) -> Result<WarningThreshold, StoreError> {
    let percent: i64 = self.connection.query_row(
      "SELECT usage_warning_threshold FROM app_settings WHERE id = 1",
      [],
      |row| row.get(0),
    )?;
    u32::try_from(percent)
      .ok()
      .and_then(|percent| WarningThreshold::new(percent).ok())
      .ok_or_else(|| corrupt("usage warning threshold", percent))
  }

  pub fn set_usage_warning_threshold(&mut self, threshold: WarningThreshold) -> Result<(), StoreError> {
    self.connection.execute(
      "UPDATE app_settings SET usage_warning_threshold = ?1 WHERE id = 1",
      params![threshold.percent()],
    )?;
    Ok(())
  }
}
