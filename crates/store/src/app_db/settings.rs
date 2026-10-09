//! Settings of the application rather than of a project: whether automation may answer waiting tool
//! requests through Orchester's MCP server (off until a person turns it on), the warning threshold the
//! Usage dashboard draws in Plan limits (limits belong to accounts, so every project shows the same
//! line; 80 % until it is changed), and `orchester-app`'s theme (dark until changed) and whether agents
//! may control its window through its MCP server (the build decides until it is set), and whether
//! automation may widen what agents may do through Orchester's MCP server (off until a person turns it on
//! in the window).

use orchester_core::{appearance::ThemeSetting, usage::limits::WarningThreshold};
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

pub(super) const APPEARANCE_MIGRATION: &str = r"
-- orchester-app: the window's theme, and window control for its MCP server (NULL: as the build decides).
ALTER TABLE app_settings ADD COLUMN theme TEXT NOT NULL DEFAULT 'dark' CHECK (theme IN ('dark', 'light', 'system'));
ALTER TABLE app_settings ADD COLUMN mcp_window_control INTEGER CHECK (mcp_window_control IN (0, 1));
";

pub(super) const AUTOMATION_PERMISSIONS_MIGRATION: &str = r"
-- Let automation change agent permissions: off until a person turns it on in the window.
ALTER TABLE app_settings ADD COLUMN automation_changes_permissions INTEGER NOT NULL DEFAULT 0
  CHECK (automation_changes_permissions IN (0, 1));
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

  /// The theme `orchester-app` shows.
  pub fn theme(&self) -> Result<ThemeSetting, StoreError> {
    let name: String = self
      .connection
      .query_row("SELECT theme FROM app_settings WHERE id = 1", [], |row| row.get(0))?;
    ThemeSetting::from_name(&name).ok_or_else(|| corrupt("theme", name))
  }

  pub fn set_theme(&mut self, theme: ThemeSetting) -> Result<(), StoreError> {
    self
      .connection
      .execute("UPDATE app_settings SET theme = ?1 WHERE id = 1", params![theme.name()])?;
    Ok(())
  }

  /// Whether agents may control `orchester-app`'s window through its MCP server; `None` until it is set,
  /// when the build decides.
  pub fn mcp_window_control(&self) -> Result<Option<bool>, StoreError> {
    Ok(
      self
        .connection
        .query_row("SELECT mcp_window_control FROM app_settings WHERE id = 1", [], |row| {
          row.get(0)
        })?,
    )
  }

  pub fn set_mcp_window_control(&mut self, allowed: Option<bool>) -> Result<(), StoreError> {
    self.connection.execute(
      "UPDATE app_settings SET mcp_window_control = ?1 WHERE id = 1",
      params![allowed],
    )?;
    Ok(())
  }

  /// Whether automation may widen what agents may do through Orchester's MCP server (*Let automation change
  /// agent permissions*): broader profile permissions, a CLI program of its choice, environment variables
  /// that change what programs load. Narrowing never needs it.
  pub fn automation_changes_permissions(&self) -> Result<bool, StoreError> {
    Ok(self.connection.query_row(
      "SELECT automation_changes_permissions FROM app_settings WHERE id = 1",
      [],
      |row| row.get(0),
    )?)
  }

  pub fn set_automation_changes_permissions(&mut self, allowed: bool) -> Result<(), StoreError> {
    self.connection.execute(
      "UPDATE app_settings SET automation_changes_permissions = ?1 WHERE id = 1",
      params![allowed],
    )?;
    Ok(())
  }
}
