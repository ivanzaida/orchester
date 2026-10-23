use std::{path::Path, time::Duration};

use rusqlite::Connection;
use tokio::process::Command;

use crate::support::TestServer;

fn host_command(directory: &Path) -> Command {
  let mut command = Command::new(env!("CARGO_BIN_EXE_orchester-server"));
  command.env("ORCHESTER_SERVER_DATA_DIR", directory).kill_on_drop(true);
  command
}

#[tokio::test]
async fn s1a_backup_cli_reads_committed_data_while_an_immediate_writer_is_held() {
  let server = TestServer::start(|_| {}).await;
  let db = server.database();
  db.execute_batch("BEGIN IMMEDIATE; UPDATE meta SET schema_version=999;")
    .expect("uncommitted writer");
  let path = server.directory.path().join("locked-snapshot.db");
  let output = tokio::time::timeout(Duration::from_secs(2), super::backup::backup_command(&server, &path))
    .await
    .expect("backup must not queue for a migration write lock");
  assert!(output.status.success());
  assert!(output.stdout.is_empty());
  db.execute_batch("ROLLBACK").expect("release writer");
  let snapshot = Connection::open(&path).expect("snapshot");
  let version: i64 = snapshot
    .query_row("SELECT schema_version FROM meta", [], |row| row.get(0))
    .expect("committed schema");
  assert_eq!(version, 7);
  let integrity: String = snapshot
    .query_row("PRAGMA integrity_check", [], |row| row.get(0))
    .expect("snapshot integrity");
  assert_eq!(integrity, "ok");
  drop(snapshot);
  drop(db);
  server.finish().await;
}

#[tokio::test]
async fn s1a_backup_cli_never_creates_or_migrates_its_source_database() {
  let server = TestServer::start(|_| {}).await;
  let directory = server.directory.path().join("missing-source");
  let mut command = host_command(&directory);
  let output = command
    .arg("backup")
    .arg(server.directory.path().join("missing.db"))
    .output()
    .await
    .expect("missing source command");
  assert!(!output.status.success());
  assert!(output.stdout.is_empty());
  assert!(!directory.exists());
  assert!(!server.directory.path().join("missing.db").exists());
  server
    .database()
    .execute("UPDATE meta SET schema_version=999", [])
    .expect("future schema");
  let path = server.directory.path().join("future.db");
  let output = super::backup::backup_command(&server, &path).await;
  assert!(output.status.success());
  let snapshot = Connection::open(path).expect("physical snapshot of future schema");
  let version: i64 = snapshot
    .query_row("SELECT schema_version FROM meta", [], |row| row.get(0))
    .expect("unmodified schema version");
  assert_eq!(version, 999);
  drop(snapshot);
  server.finish().await;
}

#[tokio::test]
async fn s1c_host_code_commands_report_writer_lock_and_print_no_codes() {
  let server = TestServer::start(|_| {}).await;
  let db = server.database();
  db.execute_batch("BEGIN IMMEDIATE").expect("held writer");
  let mut setup = host_command(server.directory.path());
  setup.arg("setup-code");
  let mut account = host_command(server.directory.path());
  account.args(["account-code", "alice"]);
  let (setup, account) = tokio::time::timeout(Duration::from_secs(8), async {
    tokio::join!(setup.output(), account.output())
  })
  .await
  .expect("bounded busy timeout");
  for output in [setup.expect("setup command"), account.expect("account command")] {
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let logs = String::from_utf8(output.stderr).expect("operator error");
    assert!(logs.contains("server database is busy or locked; retry after the writer releases the lock"));
  }
  db.execute_batch("ROLLBACK").expect("release writer");
  drop(db);
  server.finish().await;
}
