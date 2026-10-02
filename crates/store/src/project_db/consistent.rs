//! Reads that take several statements see one state of the database: outside a caller's own
//! transaction they share a read transaction, so another connection committing between two of
//! the statements (the runner writing while the desktop reads) cannot tear what they return.

use rusqlite::Connection;

pub(super) fn read<T, E: From<rusqlite::Error>>(
  connection: &Connection,
  read: impl FnOnce() -> Result<T, E>,
) -> Result<T, E> {
  let snapshot = connection
    .is_autocommit()
    .then(|| connection.unchecked_transaction())
    .transpose()?;
  let value = read()?;
  if let Some(snapshot) = snapshot {
    snapshot.commit()?;
  }
  Ok(value)
}
