use std::{
  path::PathBuf,
  time::{Duration, Instant},
};

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::support::TestServer;

async fn read_closed(stream: &mut (impl AsyncRead + Unpin)) -> Vec<u8> {
  let mut response = Vec::new();
  let result = tokio::time::timeout(Duration::from_secs(14), stream.read_to_end(&mut response))
    .await
    .expect("connection deadline");
  // Closing with unread client data can be observed as a TCP reset instead of EOF.
  if let Err(error) = result {
    assert!(matches!(
      error.kind(),
      std::io::ErrorKind::ConnectionReset | std::io::ErrorKind::UnexpectedEof
    ));
  }
  response
}

async fn slow_headers(mut stream: impl AsyncRead + AsyncWrite + Unpin) {
  let started = Instant::now();
  stream
    .write_all(b"GET /api/v1/server HTTP/1.1\r\nHost:")
    .await
    .expect("partial headers");
  // Continued activity must not restart the absolute header deadline.
  tokio::time::sleep(Duration::from_secs(6)).await;
  stream.write_all(b" local").await.expect("slow header bytes");
  let response = read_closed(&mut stream).await;
  assert!(started.elapsed() >= Duration::from_secs(9));
  assert!(started.elapsed() < Duration::from_secs(14));
  assert!(response.is_empty() || response.starts_with(b"HTTP/1.1 408"));
}

#[tokio::test]
async fn s1a_slow_headers_and_idle_keepalive_connections_close() {
  let server = TestServer::start(|_| {}).await;
  let mut idle = server.connect().await;
  idle
    .write_all(b"GET /api/v1/server HTTP/1.1\r\nHost: localhost\r\n\r\n")
    .await
    .expect("keepalive request");
  let ((), response) = tokio::join!(slow_headers(server.connect().await), read_closed(&mut idle));
  assert!(response.starts_with(b"HTTP/1.1 200"));
  assert_eq!(server.request("GET", "/server", None, None).await.status, 200);
  server.finish().await;
}

#[tokio::test]
async fn s1a_direct_tls_enforces_the_same_header_deadline() {
  let server = TestServer::start(|config| {
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    config.tls = orchester_server::config::TlsMode::Pem;
    config.tls_cert = Some(fixtures.join("server.pem"));
    config.tls_key = Some(fixtures.join("server-key.pem"));
  })
  .await;
  slow_headers(server.tls_connect().await).await;
  assert_eq!(server.tls_request().await.status, 200);
  server.finish().await;
}

#[tokio::test]
async fn s1a_body_read_has_an_absolute_deadline_and_chunked_sizes_return_413() {
  let server = TestServer::start(|_| {}).await;
  let mut slow = server.connect().await;
  let started = Instant::now();
  slow.write_all(b"POST /api/v1/auth/sessions HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: 100\r\n\r\n{").await.expect("body start");
  tokio::time::sleep(Duration::from_secs(6)).await;
  slow.write_all(b" ").await.expect("slow body bytes");
  let response = read_closed(&mut slow).await;
  assert!(started.elapsed() >= Duration::from_secs(9));
  assert!(started.elapsed() < Duration::from_secs(14));
  assert!(response.starts_with(b"HTTP/1.1 408"));
  assert!(String::from_utf8_lossy(&response).contains("request_timeout"));
  let mut chunked = server.connect().await;
  chunked.write_all(b"POST /api/v1/auth/sessions HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\nTransfer-Encoding: chunked\r\n\r\n100001\r\n").await.expect("chunked header");
  chunked
    .write_all(&vec![b'a'; 1024 * 1024 + 1])
    .await
    .expect("large chunk");
  chunked.write_all(b"\r\n0\r\n\r\n").await.expect("chunk terminator");
  let response = read_closed(&mut chunked).await;
  assert!(response.starts_with(b"HTTP/1.1 413"));
  assert!(String::from_utf8_lossy(&response).contains("payload_too_large"));
  server.finish().await;
}
