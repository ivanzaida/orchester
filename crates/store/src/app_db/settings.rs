//! Settings of the application rather than of a project: whether automation may answer waiting tool
//! requests through Orchester's MCP server (off until a person turns it on), the warning threshold the
//! Usage dashboard draws in Plan limits (limits belong to accounts, so every project shows the same
//! line; 80 % until it is changed), and `orchester-app`'s theme (dark until changed) and whether agents
//! may control its window through its MCP server (the build decides until it is set), whether
//! automation may widen what agents may do through Orchester's MCP server (off until a person turns it on
//! in the window), and the installed app's own MCP server: whether it listens, on which port, and the id of
//! this installation that names its token in the keychain.

use orchester_core::{app_mcp::McpPort, appearance::ThemeSetting, id::InstallationId, usage::limits::WarningThreshold};
use rusqlite::params;

use super::AppDb;
use crate::{
  StoreError,
  rows::{corrupt, parse},
};

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

pub(super) const APP_MCP_MIGRATION: &str = r"
-- orchester-app's MCP server at a fixed address (docs/design/app-mcp-connection.md): on until turned off, the
-- port set in Settings, and this installation's id, given on first use, which names its token in the keychain.
ALTER TABLE app_settings ADD COLUMN mcp_server_enabled INTEGER NOT NULL DEFAULT 1 CHECK (mcp_server_enabled IN (0, 1));
ALTER TABLE app_settings ADD COLUMN mcp_port INTEGER NOT NULL DEFAULT 47821 CHECK (mcp_port BETWEEN 1024 AND 65535);
ALTER TABLE app_settings ADD COLUMN installation_id TEXT;
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

  /// Whether the installed app's MCP server listens (*Let agents connect to Orchester*); on until turned off.
  pub fn mcp_server_enabled(&self) -> Result<bool, StoreError> {
    Ok(
      self
        .connection
        .query_row("SELECT mcp_server_enabled FROM app_settings WHERE id = 1", [], |row| {
          row.get(0)
        })?,
    )
  }

  pub fn set_mcp_server_enabled(&mut self, enabled: bool) -> Result<(), StoreError> {
    self.connection.execute(
      "UPDATE app_settings SET mcp_server_enabled = ?1 WHERE id = 1",
      params![enabled],
    )?;
    Ok(())
  }

  /// The port the installed app's MCP server listens on; [`McpPort::DEFAULT`] until it is changed.
  pub fn mcp_port(&self) -> Result<McpPort, StoreError> {
    let port: i64 = self
      .connection
      .query_row("SELECT mcp_port FROM app_settings WHERE id = 1", [], |row| row.get(0))?;
    u32::try_from(port)
      .ok()
      .and_then(|port| McpPort::new(port).ok())
      .ok_or_else(|| corrupt("MCP port", port))
  }

  pub fn set_mcp_port(&mut self, port: McpPort) -> Result<(), StoreError> {
    self.connection.execute(
      "UPDATE app_settings SET mcp_port = ?1 WHERE id = 1",
      params![port.get()],
    )?;
    Ok(())
  }

  /// This installation's id, given the first time it is asked for and the same from then on.
  pub fn installation_id(&mut self) -> Result<InstallationId, StoreError> {
    let given = InstallationId::generate().to_string();
    // Only where none is yet: two callers at once keep the first id.
    self.connection.execute(
      "UPDATE app_settings SET installation_id = ?1 WHERE id = 1 AND installation_id IS NULL",
      params![given],
    )?;
    let id: String = self
      .connection
      .query_row("SELECT installation_id FROM app_settings WHERE id = 1", [], |row| {
        row.get(0)
      })?;
    parse("installation id", &id)
  }
}
