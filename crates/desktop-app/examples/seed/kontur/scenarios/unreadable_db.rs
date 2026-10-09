//! `unreadable-db`: a project database written by a newer Orchester (`Tasks · load error · dark` `oDyZB`):
//! its schema version is one past what this build supports, so opening it fails with `SchemaTooNew`.

use super::empty;
use crate::kontur::{Kontur, SeedError};

pub fn seed(kontur: &mut Kontur) -> Result<(), SeedError> {
  empty::seed(kontur)?;
  // The store has no API for a newer schema: a newer build sets it, and this one only refuses it.
  let connection = rusqlite::Connection::open(kontur.database())?;
  let version: u32 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
  connection.pragma_update(None, "user_version", version + 1)?;
  Ok(())
}
