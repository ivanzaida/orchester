use std::{
  path::PathBuf,
  time::{Duration, Instant},
};

use tokio::io::AsyncReadExt;

use crate::support::TestServer;

async fn assert_other_peer_is_responsive(server: &TestServer, idle_count: usize, tls: bool) {
  let mut idle = Vec::new();
  for _ in 0..idle_count {
    idle.push(server.connect().await);
  }
  let other = "127.0.0.2".parse().expect("second loopback address");
  let response = tokio::time::timeout(Duration::from_secs(1), server.request_from(other, tls))
    .await
    .expect("other peer remains responsive");
  assert_eq!(response.status, 200);
  let mut last = idle.pop().expect("excess connection");
  let mut bytes = Vec::new();
  let result = tokio::time::timeout(Duration::from_secs(1), last.read_to_end(&mut bytes))
    .await
    .expect("excess connection rejected immediately");
  if let Err(error) = result {
    assert_eq!(error.kind(), std::io::ErrorKind::ConnectionReset);
  }
  assert!(bytes.is_empty());
  drop(idle);
}

#[tokio::test]
async fn s1a_idle_tcp_peer_cannot_fill_global_capacity_and_first_byte_expires_early() {
  let server = TestServer::start(|_| {}).await;
  assert_other_peer_is_responsive(&server, 300, false).await;
  let mut silent = server.connect_from("127.0.0.2".parse().expect("source")).await;
  let start = Instant::now();
  let mut data = Vec::new();
  tokio::time::timeout(Duration::from_secs(3), silent.read_to_end(&mut data))
    .await
    .expect("first-byte deadline")
    .expect("idle EOF");
  assert!(start.elapsed() >= Duration::from_secs(1));
  assert!(data.is_empty());
  server.finish().await;
}

#[tokio::test]
async fn s1a_silent_tls_peer_cannot_fill_handshakes_and_first_byte_expires_early() {
  let server = TestServer::start(|config| {
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    config.tls = orchester_server::config::TlsMode::Pem;
    config.tls_cert = Some(fixtures.join("server.pem"));
    config.tls_key = Some(fixtures.join("server-key.pem"));
  })
  .await;
  assert_other_peer_is_responsive(&server, 70, true).await;
  let mut silent = server.connect_from("127.0.0.2".parse().expect("source")).await;
  let start = Instant::now();
  let mut data = Vec::new();
  tokio::time::timeout(Duration::from_secs(3), silent.read_to_end(&mut data))
    .await
    .expect("TLS first-byte deadline")
    .expect("idle EOF");
  assert!(start.elapsed() >= Duration::from_secs(1));
  assert!(data.is_empty());
  server.finish().await;
}
