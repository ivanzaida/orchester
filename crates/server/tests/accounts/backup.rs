use std::{path::Path, process::Command};

use orchester_api::ServerInfo;
use rusqlite::Connection;

use crate::support::{TestServer, credential};

fn backup_command(server: &TestServer, path: &Path) -> std::process::Output {
  Command::new(env!("CARGO_BIN_EXE_orchester-server"))
    .arg("backup")
    .arg(path)
    .env("ORCHESTER_SERVER_DATA_DIR", server.directory.path())
    .output()
    .expect("backup command")
}

#[tokio::test]
async fn s1a_online_backup_cli_captures_wal_and_restores_without_overwriting_files() {
  let server = TestServer::start(|_| {}).await;
  server.bootstrap().await;
  assert!(
    server
      .directory
      .path()
      .join("orchester-server.db-wal")
      .metadata()
      .expect("live WAL")
      .len()
      > 0
  );
  let path = server.directory.path().join("snapshot.db");
  let output = backup_command(&server, &path);
  assert!(output.status.success());
  assert!(output.stdout.is_empty());
  let snapshot = Connection::open(&path).expect("snapshot database");
  let integrity: String = snapshot
    .query_row("PRAGMA integrity_check", [], |row| row.get(0))
    .expect("integrity check");
  assert_eq!(integrity, "ok");
  let audits: i64 = snapshot
    .query_row(
      "SELECT COUNT(*) FROM audit_events WHERE action='password_changed'",
      [],
      |row| row.get(0),
    )
    .expect("WAL audit copied");
  assert_eq!(audits, 1);
  drop(snapshot);
  let bytes = std::fs::read(&path).expect("snapshot bytes");
  let refused = backup_command(&server, &path);
  assert!(!refused.status.success());
  assert!(bytes == std::fs::read(&path).expect("preserved snapshot"));
  assert!(
    !backup_command(&server, &server.directory.path().join("orchester-server.db"))
      .status
      .success()
  );
  assert!(
    !backup_command(&server, &server.directory.path().join("missing/snapshot.db"))
      .status
      .success()
  );
  let restored_dir = server.directory.path().join("restored");
  std::fs::create_dir(&restored_dir).expect("restore directory");
  std::fs::copy(&path, restored_dir.join("orchester-server.db")).expect("restore snapshot");
  let restored = TestServer::start(|config| config.data_dir = restored_dir).await;
  assert!(
    !restored
      .request("GET", "/server", None, None)
      .await
      .json::<ServerInfo>()
      .needs_bootstrap
  );
  assert!(
    !restored
      .login("alice", &credential("a secure admin password"))
      .await
      .password_change_required
  );
  restored.finish().await;
  server.finish().await;
}
