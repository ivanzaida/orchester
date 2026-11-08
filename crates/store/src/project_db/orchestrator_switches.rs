//! `orchestrator_switches`, one row: whether the Orchestrator chat drives in this project (Project settings ›
//! Orchestrator, *Let the Orchestrator drive*). `drives` is `None` until it is written for the project.

use rusqlite::params;

use super::ProjectDb;
use crate::StoreError;

impl ProjectDb {
  /// Whether the project lets the Orchestrator drive; `None` until it is written for the project.
  pub fn orchestrator_drives(&self) -> Result<Option<bool>, StoreError> {
    Ok(
      self
        .connection
        .query_row("SELECT drives FROM orchestrator_switches WHERE id = 1", [], |row| {
          row.get(0)
        })?,
    )
  }

  pub fn set_orchestrator_drives(&mut self, drives: bool) -> Result<(), StoreError> {
    self.connection.execute(
      "UPDATE orchestrator_switches SET drives = ?1 WHERE id = 1",
      params![drives],
    )?;
    Ok(())
  }

  /// Whether the project lets the Orchestrator drive; one never written for the project first takes the
  /// application-wide *answer agents* value it came from (ORC-594, ORC-909), once: `application_wide` is read only
  /// then, and a project's own value always stays.
  pub fn copy_orchestrator_drives(
    &mut self,
    application_wide: impl FnOnce() -> Result<bool, StoreError>,
  ) -> Result<bool, StoreError> {
    if let Some(drives) = self.orchestrator_drives()? {
      return Ok(drives);
    }
    self.connection.execute(
      "UPDATE orchestrator_switches SET drives = ?1 WHERE id = 1 AND drives IS NULL",
      params![application_wide()?],
    )?;
    // Another copy or a person's save may have come first: what the project holds now is its value.
    Ok(self.orchestrator_drives()?.unwrap_or(false))
  }
}
