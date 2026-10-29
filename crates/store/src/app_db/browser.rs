//! Settings › Browser (`docs/design/browser.md`, "Finding the browser"): the program the person chose for agents'
//! browser, instead of the one Orchester finds installed (none until a person chooses one), and whether the Browser
//! window opens when an agent starts browsing (on until a person turns it off, the person's decision 2026-10-29). The
//! runner reads the program as it starts a project's browser ([`AppDb::open_read_only`]).

use std::path::{Path, PathBuf};

use rusqlite::params;

use super::AppDb;
use crate::StoreError;

pub(super) const MIGRATION: &str = r"
-- Settings › Browser (ORC-369): the program chosen for agents' browser (NULL: the one found installed), and Open the
-- Browser window when an agent starts browsing, on until a person turns it off.
ALTER TABLE app_settings ADD COLUMN browser_program TEXT;
ALTER TABLE app_settings ADD COLUMN opens_browser_window INTEGER NOT NULL DEFAULT 1
  CHECK (opens_browser_window IN (0, 1));
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

  /// Whether the Browser window opens, without taking focus, when an agent starts browsing.
  pub fn opens_browser_window(&self) -> Result<bool, StoreError> {
    Ok(self.connection.query_row(
      "SELECT opens_browser_window FROM app_settings WHERE id = 1",
      [],
      |row| row.get(0),
    )?)
  }

  pub fn set_opens_browser_window(&mut self, opens: bool) -> Result<(), StoreError> {
    self.connection.execute(
      "UPDATE app_settings SET opens_browser_window = ?1 WHERE id = 1",
      params![opens],
    )?;
    Ok(())
  }
}
