use std::{path::Path, time::Duration};

use rusqlite::{Connection, TransactionBehavior};

use crate::StoreError;

/// How long a write waits for the other process (desktop or runner) to release the lock.
const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

/// Opens `path` in WAL mode and applies the forward-only `migrations` not yet recorded in
/// `user_version`. A database written by a newer build is refused rather than guessed at.
pub(crate) fn open(path: &Path, migrations: &[&str]) -> Result<Connection, StoreError> {
  let mut connection = Connection::open(path)?;
  connection.pragma_update_and_check(None, "journal_mode", "WAL", |_| Ok(()))?;
  connection.busy_timeout(BUSY_TIMEOUT)?;
  // Every transaction here writes. Taking the write lock when it begins lets it wait for the
  // other writers (the desktop, the tasks of the runs) instead of failing: a transaction that
  // started reading cannot wait for the lock without risking a deadlock, so SQLite refuses it
  // at once as "database is locked".
  connection.set_transaction_behavior(TransactionBehavior::Immediate);
  // A migration that rebuilds a table must not have the drop cascade into the rows that refer
  // to it, so foreign keys are enforced only after migrating; the migrated database is checked
  // for violations before it is committed instead.
  connection.pragma_update(None, "foreign_keys", false)?;
  apply(&mut connection, migrations)?;
  connection.pragma_update(None, "foreign_keys", true)?;
  Ok(connection)
}

fn apply(connection: &mut Connection, migrations: &[&str]) -> Result<(), StoreError> {
  let found: u32 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
  let supported = u32::try_from(migrations.len()).expect("fewer than 2^32 migrations");
  if found > supported {
    return Err(StoreError::SchemaTooNew { found, supported });
  }
  if found == supported {
    return Ok(());
  }
  let transaction = connection.transaction()?;
  for migration in &migrations[found as usize..] {
    transaction.execute_batch(migration)?;
  }
  let violations: i64 = transaction.query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| row.get(0))?;
  if violations > 0 {
    return Err(StoreError::MigrationBrokeReferences(violations));
  }
  transaction.pragma_update(None, "user_version", supported)?;
  transaction.commit()?;
  Ok(())
}
