//! *Services at once* of the computer (ORC-248, `docs/architecture/agent-testing.md`, decision 3): how many services
//! every project's runs together may run at once, an application setting since it is the computer's, which each
//! project's runner reads at each start. Each project's own share of it is with the project's limits.

use orchester_core::limits::ComputerServiceSlots;
use rusqlite::params;

use super::AppDb;
use crate::{StoreError, rows::corrupt};

pub(super) const MIGRATION: &str = r"
-- Settings › General › Services at once (ORC-248): 8 until a person changes it, in an existing database too.
ALTER TABLE app_settings ADD COLUMN service_slots INTEGER NOT NULL DEFAULT 8 CHECK (service_slots BETWEEN 1 AND 32);
";

impl AppDb {
  /// The computer's *Services at once*; [`ComputerServiceSlots::DEFAULT`] until it is changed.
  pub fn service_slots(&self) -> Result<ComputerServiceSlots, StoreError> {
    let count: i64 = self
      .connection
      .query_row("SELECT service_slots FROM app_settings WHERE id = 1", [], |row| {
        row.get(0)
      })?;
    u64::try_from(count)
      .ok()
      .and_then(|count| ComputerServiceSlots::new(count).ok())
      .ok_or_else(|| corrupt("services at once", count))
  }

  pub fn set_service_slots(&mut self, slots: ComputerServiceSlots) -> Result<(), StoreError> {
    self.connection.execute(
      "UPDATE app_settings SET service_slots = ?1 WHERE id = 1",
      params![slots.get().get()],
    )?;
    Ok(())
  }
}
