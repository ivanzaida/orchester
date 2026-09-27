use std::{
  path::Path,
  thread,
  time::{Duration, Instant},
};

use rusqlite::{Connection, TransactionBehavior, ffi};

use crate::StoreError;

/// How long opening, a read or a write waits for the other process (desktop or runner) to let go
/// of the database.
const BUSY_TIMEOUT: Duration = Duration::from_secs(5);
/// The pause before opening again while a process that is going away still maps the database's
/// shared memory.
const REOPEN_PAUSE: Duration = Duration::from_millis(20);

/// Opens `path` in WAL mode and applies the forward-only `migrations` not yet recorded in
/// `user_version`. A database written by a newer build is refused rather than guessed at.
pub(crate) fn open(path: &Path, migrations: &[&str]) -> Result<Connection, StoreError> {
  let mut connection = connect(path)?;
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

/// Opens `path` in WAL mode, waiting at most [`BUSY_TIMEOUT`] while the other process holds it:
/// SQLite's busy handler waits for its locks, from the first read on. A process that crashed or
/// was killed lets go of its locks on the shared memory of the write-ahead log (`-shm`) before
/// Windows unmaps that file from it. The next process to open the database takes itself for the
/// first one and resets the file, which fails with "disk I/O error" while the file is still
/// mapped; opening is tried again until the map is gone. Every other error comes back at once.
fn connect(path: &Path) -> Result<Connection, rusqlite::Error> {
  let deadline = Instant::now() + BUSY_TIMEOUT;
  loop {
    let connection = Connection::open(path)?;
    connection.busy_timeout(BUSY_TIMEOUT)?;
    match connection.pragma_update_and_check(None, "journal_mode", "WAL", |_| Ok(())) {
      Ok(()) => return Ok(connection),
      Err(error) if shared_memory_still_mapped(&error) && Instant::now() < deadline => thread::sleep(REOPEN_PAUSE),
      Err(error) => return Err(error),
    }
  }
}

/// Whether the first read failed to reset the shared memory file: another process still maps it.
fn shared_memory_still_mapped(error: &rusqlite::Error) -> bool {
  matches!(error, rusqlite::Error::SqliteFailure(failure, _) if failure.extended_code == ffi::SQLITE_IOERR_TRUNCATE)
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
