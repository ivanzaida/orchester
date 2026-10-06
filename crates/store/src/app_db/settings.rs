//! Settings of the application rather than of a project: whether automation may answer waiting tool
//! requests through Orchester's MCP server. Off until a person turns it on.

use rusqlite::params;

use super::AppDb;
use crate::StoreError;

pub(super) const MIGRATION: &str = r"
CREATE TABLE app_settings (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  automation_answers_approvals INTEGER NOT NULL DEFAULT 0 CHECK (automation_answers_approvals IN (0, 1))
) STRICT;
INSERT INTO app_settings (id) VALUES (1);
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
}
