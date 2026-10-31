//! Settings › Browser (`docs/design/browser.md`, "Finding the browser"): the program the person chose for agents'
//! browser, instead of the one Orchester finds installed (none until a person chooses one). The runner reads it as it
//! starts a project's browser ([`AppDb::open_read_only`]). Whether the Browser window opens when an agent starts
//! browsing is [`AppDb::browser_window_opens_itself`].

use std::path::{Path, PathBuf};

use rusqlite::params;

use super::AppDb;
use crate::StoreError;

pub(super) const MIGRATION: &str = r"
-- Settings › Browser (ORC-369): the program chosen for agents' browser (NULL: the one found installed).
ALTER TABLE app_settings ADD COLUMN browser_program TEXT;
";

impl AppDb {
  /// The program the person chose for agents' browser; `None` while Orchester uses the one it finds installed.
  pub fn browser_program(&self) -> Result<Option<PathBuf>, StoreError> {
    let program: Option<String> =
      self
        .connection
        .query_row("SELECT browser_program FROM app_settings WHERE id = 1", [], |row| {
          row.get(0)
        })?;
    Ok(program.map(PathBuf::from))
  }

  /// Keeps `program` as agents' browser, or the one found installed again with `None`. Refused for a path that is not
  /// Unicode, which the database cannot keep exactly.
  pub fn set_browser_program(&mut self, program: Option<&Path>) -> Result<(), StoreError> {
    let program = program
      .map(|program| {
        program
          .to_str()
          .ok_or(StoreError::UnrepresentablePath("browser program"))
      })
      .transpose()?;
    self.connection.execute(
      "UPDATE app_settings SET browser_program = ?1 WHERE id = 1",
      params![program],
    )?;
    Ok(())
  }
}
