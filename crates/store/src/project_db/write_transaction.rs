//! A write decision and its mutation share SQLite's writer lock, including other database handles.

use super::ProjectDb;
use crate::StoreError;

impl ProjectDb {
  /// Runs `write` under an immediate transaction. The callback must not start nested transactions.
  /// Other handles and processes cannot commit bindings between its reads and external writes.
  pub fn with_write_transaction<T, E>(
    &mut self,
    write: impl FnOnce(&mut Self) -> Result<T, E>,
    database_error: impl Fn(StoreError) -> E,
  ) -> Result<T, E> {
    self
      .connection
      .execute_batch("BEGIN IMMEDIATE")
      .map_err(|error| database_error(error.into()))?;
    let transaction = WriteTransaction(self);
    let value = write(transaction.0)?;
    transaction
      .0
      .connection
      .execute_batch("COMMIT")
      .map_err(|error| database_error(error.into()))?;
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
