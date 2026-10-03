use std::{path::PathBuf, process::Command};

use orchester_api::ServerInfo;
use orchester_server::{Server, config::Config, error::ServerError};

use crate::support::{TestServer, credential};

#[tokio::test]
async fn s1a_configured_tls_uses_a_real_trusted_certificate() {
  let server = TestServer::start(|config| {
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    config.tls_cert = Some(fixtures.join("server.pem"));
    config.tls_key = Some(fixtures.join("server-key.pem"));
  })
  .await;
  let response = server.tls_request().await;
  assert_eq!(response.status, 200);
  assert!(response.json::<ServerInfo>().needs_bootstrap);
  server.finish().await;
}

#[tokio::test]
async fn s1a_host_cli_creates_and_resets_while_the_server_is_running() {
  let server = TestServer::start(|_| {}).await;
  let binary = env!("CARGO_BIN_EXE_orchester-server");
  let output = Command::new(binary)
    .args(["user", "create", "--admin", "cli_admin"])
    .env("ORCHESTER_SERVER_DATA_DIR", server.directory.path())
    .output()
    .expect("create command");
  assert!(output.status.success());
  let initial = credential(std::str::from_utf8(&output.stdout).expect("stdout").trim());
  assert!(!String::from_utf8_lossy(&output.stderr).contains(initial.expose()));
  assert!(server.login("cli_admin", &initial).await.password_change_required);
  let reset = Command::new(binary)
    .args(["user", "reset-password", "cli_admin"])
    .env("ORCHESTER_SERVER_DATA_DIR", server.directory.path())
    .output()
    .expect("reset command");
  assert!(reset.status.success());
  let replacement = credential(std::str::from_utf8(&reset.stdout).expect("stdout").trim());
  assert!(replacement.expose() != initial.expose());
  assert!(!String::from_utf8_lossy(&reset.stderr).contains(replacement.expose()));
  assert!(server.login("cli_admin", &replacement).await.password_change_required);
  let failed = Command::new(binary)
    .args(["user", "create", "--admin", "cli_admin"])
    .env("ORCHESTER_SERVER_DATA_DIR", server.directory.path())
    .output()
    .expect("duplicate command");
  assert!(!failed.status.success());
  assert!(failed.stdout.is_empty());
  server.finish().await;
}

#[tokio::test]
async fn s1a_invalid_configuration_and_future_schema_are_refused() {
  let server = TestServer::start(|_| {}).await;
  let invalid = Config {
    data_dir: server.directory.path().into(),
    tls_cert: Some("missing.pem".into()),
    ..Config::default()
  };
  assert!(matches!(
    Server::open(invalid, server.clock.clone()).await,
    Err(ServerError::Configuration(_))
  ));
  let database = server.database();
  database
    .execute("UPDATE meta SET schema_version=999", [])
    .expect("future schema fixture");
  let config = Config {
    data_dir: server.directory.path().into(),
    ..Config::default()
  };
  assert!(matches!(
    Server::open(config, server.clock.clone()).await,
    Err(ServerError::Configuration(_))
  ));
  drop(database);
  server.finish().await;
}
