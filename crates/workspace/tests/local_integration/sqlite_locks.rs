//! Checking the store keeps the locks this process's SQLite connections hold on the project database (ORC-630). On
//! macOS a process's record locks on a file are dropped as soon as it closes any descriptor of that file, one SQLite
//! does not know of too. A check that opened `orchester.db` and `orchester.db-shm` dropped the app's locks while its
//! connections stayed open: the runner, taking itself for the last connection, then deleted the log and the shared
//! memory, or, taking itself for the first, cut the shared memory the app still had mapped, and the app's next read
//! of it ended in SIGBUS.

use std::{env, fs, path::PathBuf, process::Command};

use rusqlite::Connection;

/// Names the database [`local_integration_sqlite_second_process_opens_and_closes_the_database`] opens, when a test
/// starts it as the other process.
const SECOND_PROCESS: &str = "ORCHESTER_TEST_SQLITE_SECOND_PROCESS";
const SECOND_PROCESS_TEST: &str = "sqlite_locks::local_integration_sqlite_second_process_opens_and_closes_the_database";

#[test]
fn local_integration_store_check_keeps_the_sqlite_locks_of_this_process() {
  let scratch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.tmp/orc-24-tests");
  fs::create_dir_all(&scratch).expect("scratch");
  let repo = tempfile::Builder::new()
    .prefix("sqlite-locks-")
    .tempdir_in(scratch)
    .expect("repo");
  let directory = repo.path().join(".orchester");
  fs::create_dir(&directory).expect("project data folder");
  let database = directory.join("orchester.db");
  let wal = directory.join("orchester.db-wal");
  let shared_memory = directory.join("orchester.db-shm");
  // As the app: a connection in WAL mode stays open, with the log and the shared memory it maps.
  let app = Connection::open(&database).expect("the app's connection");
  app
    .execute_batch("PRAGMA journal_mode = WAL; CREATE TABLE note (value INTEGER); INSERT INTO note VALUES (1);")
    .expect("a write into the log");
  assert_eq!(read(&app), 1);
  assert!(
    wal.is_file() && shared_memory.is_file(),
    "the app's log and shared memory"
  );

  drop(orchester_exec::local_git::owned::StoreOwnership::acquire(repo.path()).expect("the store is checked"));

  // As the runner: another process opens the database and closes its last connection. SQLite there checkpoints and
  // deletes the log and the shared memory only when no other process holds a lock on the database.
  let second = Command::new(env::current_exe().expect("this test program"))
    .args([SECOND_PROCESS_TEST, "--exact", "--test-threads=1", "--nocapture"])
    .env(SECOND_PROCESS, &database)
    .output()
    .expect("the second process");
  let report = String::from_utf8_lossy(&second.stdout);
  assert!(second.status.success(), "the second process failed: {report}");
  assert!(report.contains("1 passed"), "the second process ran no test: {report}");
  assert!(
    wal.is_file() && shared_memory.is_file(),
    "the second process took itself for the last one: the app's locks were gone"
  );
  assert_eq!(read(&app), 1, "the app still reads");
}

/// The other process of [`local_integration_store_check_keeps_the_sqlite_locks_of_this_process`]; nothing to do when
/// the test suite runs it.
#[test]
fn local_integration_sqlite_second_process_opens_and_closes_the_database() {
  let Some(database) = env::var_os(SECOND_PROCESS) else {
    return;
  };
  let connection = Connection::open(database).expect("the second connection");
  assert_eq!(read(&connection), 1);
  connection.close().map_err(|(_, error)| error).expect("closed");
}

fn read(connection: &Connection) -> i64 {
  connection
    .query_row("SELECT value FROM note", [], |row| row.get(0))
    .expect("the note")
}
