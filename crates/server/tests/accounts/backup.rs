use std::{path::Path, time::Duration};

use orchester_api::ServerInfo;
use rusqlite::Connection;
use tokio::process::Command;

use crate::support::TestServer;

pub(super) async fn backup_command(server: &TestServer, path: &Path) -> std::process::Output {
  tokio::time::timeout(
    Duration::from_secs(8),
    Command::new(env!("CARGO_BIN_EXE_orchester-server"))
      .arg("backup")
      .arg(path)
      .env("ORCHESTER_SERVER_DATA_DIR", server.directory.path())
      .kill_on_drop(true)
      .output(),
  )
  .await
  .expect("backup command deadline")
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
  let output = backup_command(&server, &path).await;
  assert!(output.status.success());
  assert!(output.stdout.is_empty());
  let snapshot = Connection::open(&path).expect("snapshot database");
  let integrity: String = snapshot
    .query_row("PRAGMA integrity_check", [], |row| row.get(0))
    .expect("integrity check");
  assert_eq!(integrity, "ok");
  let audits: i64 = snapshot
    .query_row("SELECT COUNT(*) FROM audit_events WHERE action='joined'", [], |row| {
      row.get(0)
    })
    .expect("WAL audit copied");
  assert_eq!(audits, 1);
  drop(snapshot);
  let bytes = std::fs::read(&path).expect("snapshot bytes");
  let refused = backup_command(&server, &path).await;
  assert!(!refused.status.success());
  assert!(bytes == std::fs::read(&path).expect("preserved snapshot"));
  assert!(
    !backup_command(&server, &server.directory.path().join("orchester-server.db"))
      .await
      .status
      .success()
  );
  assert!(
    !backup_command(&server, &server.directory.path().join("missing/snapshot.db"))
      .await
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
  let code = restored
    .server
    .host_account_code("alice".into())
    .await
    .expect("account code");
  assert_eq!(
    restored.join_as(&code, "alice", "restored laptop").await.user.username,
    "alice"
  );
  restored.finish().await;
  server.finish().await;
}
