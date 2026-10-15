use std::{path::PathBuf, sync::Arc, time::Duration};

use orchester_server::config::Config;
use tokio::{
  io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
  task::JoinSet,
};

use crate::support::TestServer;

fn direct_tls(config: &mut Config) {
  let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
  config.tls = orchester_server::config::TlsMode::Pem;
  config.tls_cert = Some(fixtures.join("server.pem"));
  config.tls_key = Some(fixtures.join("server-key.pem"));
}

async fn slow_body(stream: &mut (impl AsyncWrite + Unpin), client: usize) {
  let headers = format!(
    "POST /api/v1/auth/sessions HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\nContent-Length: 128\r\nX-Forwarded-For: 203.0.113.{client}\r\n\r\n"
  );
  stream.write_all(headers.as_bytes()).await.expect("slow-body headers");
}

async fn refused(stream: &mut (impl AsyncRead + Unpin)) {
  let mut bytes = Vec::new();
  let result = tokio::time::timeout(Duration::from_secs(1), stream.read_to_end(&mut bytes))
    .await
    .expect("excess connection closed immediately");
  if let Err(error) = result {
    assert_eq!(error.kind(), std::io::ErrorKind::ConnectionReset);
  }
  assert!(bytes.is_empty());
}

#[tokio::test]
async fn s1a_trusted_proxy_serves_more_than_64_clients_while_untrusted_peer_stays_capped() {
  let server =
    TestServer::start(|config| config.trusted_proxies = vec!["127.0.0.1/32".parse().expect("proxy CIDR")]).await;
  let mut proxy = Vec::new();
  let mut clients = Vec::new();
  for client in 1..=64 {
    let mut stream = server.connect().await;
    slow_body(&mut stream, client).await;
    proxy.push(stream);
    let mut stream = server
      .connect_from("127.0.0.2".parse().expect("untrusted address"))
      .await;
    slow_body(&mut stream, client).await;
    clients.push(stream);
  }
  let response = tokio::time::timeout(
    Duration::from_secs(1),
    server.request_headers("GET", "/server", None, None, &[("X-Forwarded-For", "198.51.100.7")]),
  )
  .await
  .expect("65th forwarded client remains responsive");
  assert_eq!(response.status, 200);
  let mut excess = server
    .connect_from("127.0.0.2".parse().expect("untrusted address"))
    .await;
  refused(&mut excess).await;
  drop(proxy);
  drop(clients);
  server.finish().await;
}

#[tokio::test]
async fn s1a_trusted_proxy_connections_still_obey_the_global_cap() {
  let server = TestServer::start(|config| config.trusted_proxies = vec!["127.0.0.1".parse().expect("proxy")]).await;
  let mut streams = Vec::new();
  for client in 0..256 {
    let mut stream = server.connect().await;
    slow_body(&mut stream, client).await;
    streams.push(stream);
  }
  let mut excess = server.connect().await;
  refused(&mut excess).await;
  drop(streams);
  server.finish().await;
}

#[tokio::test]
async fn s1a_trusted_tls_proxy_bypasses_peer_caps_but_not_the_global_handshake_cap() {
  let server = TestServer::start(|config| {
    direct_tls(config);
    config.trusted_proxies = vec!["::ffff:127.0.0.1".parse().expect("mapped proxy")];
  })
  .await;
  let mut streams = Vec::new();
  for client in 0..80 {
    let mut stream = server.tls_connect().await;
    slow_body(&mut stream, client).await;
    streams.push(stream);
  }
  let mut silent = Vec::new();
  for _ in 0..50 {
    silent.push(server.connect().await);
  }
  let response = tokio::time::timeout(Duration::from_secs(1), server.tls_request())
    .await
    .expect("trusted proxy exceeds both peer caps");
  assert_eq!(response.status, 200);
  for _ in 50..64 {
    silent.push(server.connect().await);
  }
  let mut excess = server.connect().await;
  refused(&mut excess).await;
  drop(silent);
  drop(streams);
  server.finish().await;
}

#[tokio::test]
async fn s1a_nat_peer_can_complete_32_concurrent_tls_handshakes_and_rejects_the_33rd() {
  let server = Arc::new(TestServer::start(direct_tls).await);
  let mut pending = Vec::new();
  for _ in 0..31 {
    pending.push(server.connect().await);
  }
  assert_eq!(
    tokio::time::timeout(Duration::from_secs(1), server.tls_request())
      .await
      .expect("32nd handshake admitted")
      .status,
    200
  );
  pending.push(server.connect().await);
  let mut excess = server.connect().await;
  refused(&mut excess).await;
  let mut tasks = JoinSet::new();
  for stream in pending {
    let server = server.clone();
    tasks.spawn(async move { server.tls_on(stream).await });
  }
  let mut completed = Vec::new();
  while let Some(stream) = tasks.join_next().await {
    completed.push(stream.expect("admitted TLS handshake completes"));
  }
  assert_eq!(completed.len(), 32);
  drop(completed);
  Arc::try_unwrap(server)
    .unwrap_or_else(|_| panic!("workers released fixture references"))
    .finish()
    .await;
}
