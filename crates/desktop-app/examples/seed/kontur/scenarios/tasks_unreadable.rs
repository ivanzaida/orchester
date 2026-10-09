//! `tasks-unreadable`: a project that opens, with a task whose stored record cannot be read back
//! (`Tasks · load error · dark` `oDyZB`): its status is not one this build knows, as an edit outside
//! Orchester would leave it, so reading the task list fails with `Corrupt` while the project itself opens.

use crate::kontur::{Kontur, SeedError};

pub fn seed(kontur: &mut Kontur) -> Result<(), SeedError> {
  kontur.catalogue()?;
  let at = kontur.ago(60);
  let task = kontur.task(("Add retry to webhook sender", "", &[]), None, at)?;
  kontur.seeded(&task, "status unreadable");
  // The store has no API for a damaged record: an edit outside Orchester leaves it.
  let changed = rusqlite::Connection::open(kontur.database())?.execute(
    "UPDATE tasks SET status = 'not a status' WHERE id = ?1",
    [task.id().to_string()],
  )?;
  if changed != 1 {
    return Err(SeedError::Invalid(format!(
      "damaging task {} changed {changed} rows",
      task.key()
    )));
  }
  Ok(())
}
