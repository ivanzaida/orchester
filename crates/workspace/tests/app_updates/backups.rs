//! The databases copied aside before an update installs (`app_updates::install::back_up`): where the copies go, the
//! last two backups kept, and each copy taken through SQLite (ORC-631): a valid database on its own, holding what was
//! still in the write-ahead log, taken without dropping the locks this process's own connections hold.
//!
//! `cargo test -p orchester-workspace --test app_updates update_install_backup`

use std::{
  fs,
  path::{Path, PathBuf},
  process::Command,
};

use orchester_workspace::app_updates::install::{BACKUPS_FOLDER, Databases, back_up, backups};
use rusqlite::{Connection, OpenFlags};

use super::{RUNNING, version};

/// A database at `path` in WAL mode, as the app and the runners keep theirs, holding `note`; the connection stays open
/// with the note in the write-ahead log only, not checkpointed into the database file.
fn database(path: &Path, note: &str) -> Connection {
  fs::create_dir_all(path.parent().expect("a folder")).expect("the folder");
  let connection = Connection::open(path).expect("a database");
  connection
    .execute_batch("PRAGMA journal_mode = WAL; PRAGMA wal_autocheckpoint = 0; CREATE TABLE note (value TEXT);")
    .expect("the table");
  set_note(&connection, note);
  connection
}

fn set_note(connection: &Connection, note: &str) {
  connection
    .execute_batch("DELETE FROM note")
    .expect("the old note removed");
  connection
    .execute("INSERT INTO note VALUES (?1)", [note])
    .expect("the note");
}

fn note(connection: &Connection) -> String {
  connection
    .query_row("SELECT value FROM note", [], |row| row.get(0))
    .expect("the note")
}

/// The note of the database copied to `path`, opened on its own.
fn copied_note(path: &Path) -> String {
  let copy = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY).expect("the copy opens");
  let check: String = copy
    .query_row("PRAGMA integrity_check", [], |row| row.get(0))
    .expect("checked");
  assert_eq!(check, "ok", "the copy is a valid database");
  note(&copy)
}

/// A project repository with a database whose note is still in its write-ahead log.
fn project(root: &Path, name: &str, value: &str) -> (PathBuf, Connection) {
  let repo = root.join(name);
  let connection = database(&repo.join(".orchester").join("orchester.db"), value);
  (repo, connection)
}

#[test]
fn update_install_backup_copies_the_databases_aside_and_keeps_the_last_two() {
  let data = tempfile::tempdir().expect("a folder");
  let app = data.path().join("orchester.db");
  let _app = database(&app, "app database");
  let repos = tempfile::tempdir().expect("a folder");
  let (first, _kontur) = project(repos.path(), "kontur", "kontur database");
  let empty = repos.path().join("no-database");
  fs::create_dir_all(&empty).expect("a folder");
  let databases = Databases {
    app: Some(app),
    projects: vec![first.clone(), empty],
  };

  for running in ["0.1.0", "0.2.0", "0.3.0"] {
    back_up(data.path(), &version(running), &databases).expect("backed up");
  }

  assert_eq!(
    backups(data.path()).expect("the backups"),
    [version("0.2.0"), version("0.3.0")],
    "the last two backups stay"
  );
  let folder = data.path().join(BACKUPS_FOLDER).join("0.3.0");
  assert_eq!(copied_note(&folder.join("orchester.db")), "app database");
  let kontur = folder.join("projects").join("1-kontur");
  assert_eq!(copied_note(&kontur.join("orchester.db")), "kontur database");
  let list = fs::read_to_string(folder.join("projects.txt")).expect("the list");
  assert_eq!(
    list,
    format!("1-kontur\t{}\n", first.display()),
    "a project without a database is left out"
  );
}

#[test]
fn update_install_backup_a_second_one_of_one_version_replaces_the_first() {
  let data = tempfile::tempdir().expect("a folder");
  let app = data.path().join("orchester.db");
  let databases = Databases {
    app: Some(app.clone()),
    projects: Vec::new(),
  };
  let connection = database(&app, "before");
  back_up(data.path(), &version(RUNNING), &databases).expect("backed up");
  set_note(&connection, "after");

  let folder = back_up(data.path(), &version(RUNNING), &databases).expect("backed up again");

  assert_eq!(copied_note(&folder.join("orchester.db")), "after");
  assert!(!folder.join("projects.txt").exists());
}

/// ORC-631: the copy holds what was written just before it, still only in the write-ahead log, as one database file
/// that opens on its own.
#[test]
fn update_install_backup_holds_what_is_still_in_the_write_ahead_log() {
  let data = tempfile::tempdir().expect("a folder");
  let app = data.path().join("orchester.db");
  let connection = database(&app, "written just before");
  let wal = data.path().join("orchester.db-wal");
  assert!(fs::metadata(&wal).expect("the log").len() > 0, "the note is in the log");
  let databases = Databases {
    app: Some(app),
    projects: Vec::new(),
  };

  let folder = back_up(data.path(), &version(RUNNING), &databases).expect("backed up");

  assert!(
    !folder.join("orchester.db-wal").exists(),
    "the copy needs no log beside it"
  );
  assert_eq!(copied_note(&folder.join("orchester.db")), "written just before");
  assert_eq!(note(&connection), "written just before", "the app still reads");
}

/// Names the database [`update_install_backup_second_process_takes_the_database`] opens, when a test starts it as the
/// other process.
const SECOND_PROCESS: &str = "ORCHESTER_TEST_BACKUP_SECOND_PROCESS";
const SECOND_PROCESS_TEST: &str = "backups::update_install_backup_second_process_takes_the_database";

/// ORC-631: backing up keeps the locks this process's SQLite connections hold on the databases. On macOS a process's
/// record locks on a file are dropped as soon as it closes any descriptor of that file, one SQLite does not know of
/// too; a backup that copied `orchester.db` with `fs::copy` dropped the shared lock the app's connection holds on it
/// in WAL mode while that connection stayed open, and another process could then lock the whole database (ORC-630).
#[test]
fn update_install_backup_keeps_the_sqlite_locks_of_this_process() {
  let repos = tempfile::tempdir().expect("a folder");
  let (repo, app) = project(repos.path(), "kontur", "held by the app");
  let directory = repo.join(".orchester");
  let database = directory.join("orchester.db");
  let wal = directory.join("orchester.db-wal");
  let shared_memory = directory.join("orchester.db-shm");
  assert!(
    wal.is_file() && shared_memory.is_file(),
    "the app's log and shared memory"
  );
  let data = tempfile::tempdir().expect("a folder");
  let databases = Databases {
    app: Some(database.clone()),
    projects: vec![repo],
  };

  back_up(data.path(), &version(RUNNING), &databases).expect("backed up");

  // As the runner: another process tries the lock only the database's last connection gets, then opens the database
  // and closes its last connection. SQLite there checkpoints and deletes the log and the shared memory only when no
  // other process holds a lock on the database.
  let second = Command::new(std::env::current_exe().expect("this test program"))
    .args([SECOND_PROCESS_TEST, "--exact", "--test-threads=1", "--nocapture"])
    .env(SECOND_PROCESS, &database)
    .output()
    .expect("the second process");
  let report = format!(
    "{}{}",
    String::from_utf8_lossy(&second.stdout),
    String::from_utf8_lossy(&second.stderr)
  );
  assert!(second.status.success(), "the second process failed: {report}");
  assert!(report.contains("1 passed"), "the second process ran no test: {report}");
  assert!(
    wal.is_file() && shared_memory.is_file(),
    "the second process took itself for the last one: the app's locks were gone"
  );
  assert_eq!(note(&app), "held by the app", "the app still reads");
}

/// The other process of [`update_install_backup_keeps_the_sqlite_locks_of_this_process`]; nothing to do when the test
/// suite runs it.
#[test]
fn update_install_backup_second_process_takes_the_database() {
  let Some(database) = std::env::var_os(SECOND_PROCESS) else {
    return;
  };
  // A write lock on the whole file, as SQLite's exclusive lock takes a write lock on the bytes every connection's
  // shared lock reads. Unix only: Windows locks by handle, and a backup's handle takes no lock of SQLite's with it.
  #[cfg(unix)]
  {
    let file = fs::OpenOptions::new()
      .read(true)
      .write(true)
      .open(&database)
      .expect("the database file");
    assert!(
      rustix::fs::fcntl_lock(&file, rustix::fs::FlockOperation::NonBlockingLockExclusive).is_err(),
      "the second process took the database: the app's lock on it was gone"
    );
  }
  let connection = Connection::open(&database).expect("the second connection");
  assert_eq!(note(&connection), "held by the app");
  connection.close().map_err(|(_, error)| error).expect("closed");
}
