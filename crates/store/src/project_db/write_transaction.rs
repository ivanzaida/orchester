//! A write decision and its mutation share SQLite's writer lock, including other database handles.

use rusqlite::{Connection, Transaction};

use super::ProjectDb;
use crate::{
  StoreError,
  busy::{self, Contention},
};

/// Runs `write` in an immediate transaction and commits it. A busy error anywhere in it rolls it back
/// and runs it again from the start, with growing pauses, for up to the time a write may wait for the
/// database ([`busy::retry`]), logging each retry with `what`. So `write` changes nothing but the
/// database. The run's log takes this way: its events, transcript, invocations and step results.
pub(super) fn retried<T, E>(
  connection: &mut Connection,
  what: &'static str,
  mut write: impl FnMut(&Transaction<'_>) -> Result<T, E>,
) -> Result<T, E>
where
  E: From<rusqlite::Error> + Contention,
{
  busy::retry(what, || {
    let transaction = connection.transaction()?;
    #[cfg(feature = "test-support")]
    busy::injected_failure(what)?;
    let value = write(&transaction)?;
    transaction.commit()?;
    Ok(value)
  })
}

impl ProjectDb {
  /// Runs `write` under an immediate transaction. The callback must not start nested transactions.
  /// Other handles and processes cannot commit bindings between its reads and external writes.
  /// A commit failure is mapped with the callback's outcome, including any completed external writes.
  pub fn with_write_transaction<T, E>(
    &mut self,
    write: impl FnOnce(&mut Self) -> Result<T, E>,
    begin_error: impl FnOnce(StoreError) -> E,
    commit_error: impl FnOnce(T, StoreError) -> E,
  ) -> Result<T, E> {
    self
      .connection
      .execute_batch("BEGIN IMMEDIATE")
      .map_err(|error| begin_error(error.into()))?;
    let transaction = WriteTransaction(self);
    let value = write(transaction.0)?;
    if let Err(error) = transaction.0.connection.execute_batch("COMMIT") {
      return Err(commit_error(value, error.into()));
    }
    Ok(value)
  }
}

struct WriteTransaction<'a>(&'a mut ProjectDb);

impl Drop for WriteTransaction<'_> {
  fn drop(&mut self) {
    if !self.0.connection.is_autocommit() {
      // Preserve the callback/commit error, including during unwinding. If rollback also fails,
      // SQLite rolls the transaction back when the owning connection closes.
      let _ = self.0.connection.execute_batch("ROLLBACK");
    }
  }
}
