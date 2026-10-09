use std::{
  path::Path,
  thread,
  time::{Duration, Instant},
};

use rusqlite::{Connection, OpenFlags, TransactionBehavior, ffi};

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

/// Opens `path` only to read it: SQLite refuses every write through the connection, and no
/// migration runs. The database must be at the schema `migrations` lead to; an older one has to be
/// opened with [`open`] first, by whoever owns it.
pub(crate) fn open_read_only(path: &Path, migrations: &[&str]) -> Result<Connection, StoreError> {
  let flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX;
  let (connection, found) = connect_with(path, flags, schema_version)?;
  let supported = u32::try_from(migrations.len()).expect("fewer than 2^32 migrations");
  if found > supported {
    return Err(StoreError::SchemaTooNew { found, supported });
  }
  if found < supported {
    return Err(StoreError::SchemaTooOld { found, supported });
  }
  Ok(connection)
}

/// Opens `path` in WAL mode; see [`connect_with`].
fn connect(path: &Path) -> Result<Connection, rusqlite::Error> {
  let (connection, ()) = connect_with(path, OpenFlags::default(), |connection| {
    connection.pragma_update_and_check(None, "journal_mode", "WAL", |_| Ok(()))
  })?;
  Ok(connection)
}

/// Opens `path` with `flags` and runs `first` on it, waiting at most [`BUSY_TIMEOUT`] while the
/// other process holds it: SQLite's busy handler waits for its locks, from the first read on. A
/// process that crashed or was killed lets go of its locks on the shared memory of the write-ahead
/// log (`-shm`) before Windows unmaps that file from it. The next process to open the database
/// takes itself for the first one and resets the file, which fails with "disk I/O error" while the
/// file is still mapped; opening is tried again until the map is gone. Every other error comes back
/// at once.
fn connect_with<T>(
  path: &Path,
  flags: OpenFlags,
  first: impl Fn(&Connection) -> Result<T, rusqlite::Error>,
) -> Result<(Connection, T), rusqlite::Error> {
  let deadline = Instant::now() + BUSY_TIMEOUT;
  loop {
    let connection = Connection::open_with_flags(path, flags)?;
    connection.busy_timeout(BUSY_TIMEOUT)?;
    match first(&connection) {
      Ok(value) => return Ok((connection, value)),
      Err(error) if shared_memory_still_mapped(&error) && Instant::now() < deadline => thread::sleep(REOPEN_PAUSE),
      Err(error) => return Err(error),
    }
  }
}

/// Whether the first read failed to reset the shared memory file: another process still maps it.
fn shared_memory_still_mapped(error: &rusqlite::Error) -> bool {
  matches!(error, rusqlite::Error::SqliteFailure(failure, _) if failure.extended_code == ffi::SQLITE_IOERR_TRUNCATE)
}

fn schema_version(connection: &Connection) -> Result<u32, rusqlite::Error> {
  connection.pragma_query_value(None, "user_version", |row| row.get(0))
}

fn apply(connection: &mut Connection, migrations: &[&str]) -> Result<(), StoreError> {
  let found = schema_version(connection)?;
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
  let references: i64 = transaction.query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| row.get(0))?;
  if references > 0 {
    return Err(StoreError::MigrationBrokeReferences {
      references,
      found,
      supported,
    });
  }
  transaction.pragma_update(None, "user_version", supported)?;
  transaction.commit()?;
  Ok(())
}
