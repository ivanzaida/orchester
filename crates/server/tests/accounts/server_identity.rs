//! Security review 1, B1: a team server's id is its own key's. Its link statement names that id and key and is
//! usable once; a stored id that is not its key's becomes its key's when the server opens; a token for another id is
//! refused.

use base64::Engine as _;
use orchester_api::{ErrorCode, ServerInfo, identity::PublicKey};
use orchester_server::{Server, config::Config};
use serde_json::{Value, json};

use crate::{
  cloud::{bob, cloud_join, linked, refused},
  support::TestServer,
};

fn payload(statement: &str) -> Value {
  let part = statement.split('.').nth(1).expect("JWS payload");
  let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
    .decode(part)
    .expect("base64url payload");
  serde_json::from_slice(&bytes).expect("JSON payload")
}

async fn statement(server: &TestServer, token: &orchester_api::Credential) -> Value {
  let started: Value = server
    .request("PATCH", "/server/cloud", Some(token), Some(json!({"link": "start"})))
    .await
    .json();
  payload(started["statement"].as_str().expect("a statement"))
}

#[tokio::test]
async fn sa_a_servers_id_is_its_keys_and_each_statement_is_new() {
  let server = TestServer::start(|_| {}).await;
  let admin = server.bootstrap().await;
  let first = statement(&server, &admin.token).await;
  let second = statement(&server, &admin.token).await;
  let key = PublicKey::parse(first["public_key"].as_str().expect("key")).expect("an Ed25519 key");
  assert_eq!(first["server"].as_str(), Some(key.server_id().to_string().as_str()));
  let nonce = first["nonce"].as_str().expect("a nonce");
  assert!(nonce.len() >= 16);
  assert_ne!(Some(nonce), second["nonce"].as_str(), "every statement is usable once");
  server.finish().await;
}

/// Security review 2, 3: a schema 4 database of an earlier server admin build holds an id that is not its key's. The
/// server opens with its key's id instead of refusing to start.
#[tokio::test]
async fn sa_a_stored_id_that_is_not_the_keys_becomes_the_keys_when_the_server_opens() {
  let server = TestServer::start(|_| {}).await;
  let stored = || -> String {
    server
      .database()
      .query_row("SELECT server_id FROM server_keys", [], |row| row.get(0))
      .expect("the stored id")
  };
  let own = stored();
  server
    .database()
    .execute(
      "UPDATE server_keys SET server_id=?1",
      [orchester_core::id::TeamServerId::generate().to_string()],
    )
    .expect("an earlier build's id");
  // Opened again on the same data, as a host command opens it.
  let config = Config {
    data_dir: server.directory.path().to_path_buf(),
    tls: orchester_server::config::TlsMode::Off,
    ..Config::default()
  };
  Server::open(config, server.clock.clone())
    .await
    .expect("opens with its key's id");
  assert_eq!(stored(), own, "the id is its key's again");
  server.finish().await;
}

#[tokio::test]
async fn sa_a_token_for_another_servers_id_is_refused() {
  let (server, _admin, cloud, audience) = linked().await;
  let info: ServerInfo = server.request("GET", "/server", None, None).await.json();
  assert_eq!(info.cloud.expect("linked").server, audience);
  let other = orchester_core::id::TeamServerId::generate().to_string();
  let token = cloud.token(&bob(&other, None), server.seconds());
  refused(
    &cloud_join(&server, &token, None).await,
    401,
    ErrorCode::InvalidIdentity,
  );
  server.finish().await;
}
