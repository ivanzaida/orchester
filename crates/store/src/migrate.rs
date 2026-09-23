use std::{path::Path, time::Duration};

use rusqlite::Connection;

use crate::StoreError;

/// How long a write waits for the other process (desktop or runner) to release the lock.
const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

/// Opens `path` in WAL mode and applies the forward-only `migrations` not yet recorded in
/// `user_version`. A database written by a newer build is refused rather than guessed at.
pub(crate) fn open(path: &Path, migrations: &[&str]) -> Result<Connection, StoreError> {
  let mut connection = Connection::open(path)?;
  connection.pragma_update_and_check(None, "journal_mode", "WAL", |_| Ok(()))?;
  connection.pragma_update(None, "foreign_keys", true)?;
  connection.busy_timeout(BUSY_TIMEOUT)?;
  apply(&mut connection, migrations)?;
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
  transaction.pragma_update(None, "user_version", supported)?;
  transaction.commit()?;
  Ok(())
}
