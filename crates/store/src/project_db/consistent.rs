//! Reads that take several statements see one state of the database: outside a caller's own
//! transaction they share a read transaction, so another connection committing between two of
//! the statements (the runner writing while the desktop reads) cannot tear what they return.
//! The read transaction is deferred, not the connection's immediate default: in WAL mode it reads
//! a snapshot without the write lock, so it neither waits for a writer nor holds one up.

use rusqlite::{Connection, Transaction, TransactionBehavior};

pub(super) fn read<T, E: From<rusqlite::Error>>(
  connection: &Connection,
  read: impl FnOnce() -> Result<T, E>,
) -> Result<T, E> {
  let snapshot = connection
    .is_autocommit()
    .then(|| Transaction::new_unchecked(connection, TransactionBehavior::Deferred))
    .transpose()?;
  let value = read()?;
  if let Some(snapshot) = snapshot {
    snapshot.commit()?;
  }
  Ok(value)
}
