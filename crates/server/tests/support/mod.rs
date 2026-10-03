use std::{
  collections::HashMap,
  net::SocketAddr,
  sync::{
    Arc,
    atomic::{AtomicI64, Ordering},
  },
  time::Duration,
};

use orchester_api::{CreateUser, Credential, SignedIn};
use orchester_server::{Server, clock::Clock, config::Config};
use serde::de::DeserializeOwned;
use serde_json::json;
use tempfile::TempDir;
use tokio::{
  io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
  net::{TcpListener, TcpStream},
  sync::oneshot,
  task::JoinHandle,
};

pub struct TestClock(AtomicI64);

impl TestClock {
  pub fn advance(&self, millis: i64) {
    self.0.fetch_add(millis, Ordering::SeqCst);
  }
}

impl Clock for TestClock {
  fn now_ms(&self) -> i64 {
    self.0.load(Ordering::SeqCst)
  }
}

pub struct TestServer {
  pub server: Server,
  pub directory: TempDir,
  pub clock: Arc<TestClock>,
  address: SocketAddr,
  stop: Option<oneshot::Sender<()>>,
  task: Option<JoinHandle<Result<(), orchester_server::error::ServerError>>>,
}

impl TestServer {
  pub async fn start(configure: impl FnOnce(&mut Config)) -> Self {
    let scratch = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.tmp/server-s1a/tests");
    std::fs::create_dir_all(&scratch).expect("scratch directory");
    let directory = tempfile::Builder::new()
      .prefix("s1a-")
      .tempdir_in(&scratch)
      .expect("temporary data");
    let mut config = Config {
      data_dir: directory.path().into(),
      listen: "127.0.0.1:0".parse().expect("loopback"),
      ..Config::default()
    };
    configure(&mut config);
    let clock = Arc::new(TestClock(AtomicI64::new(1_791_000_000_000)));
    let server = Server::open(config.clone(), clock.clone()).await.expect("open server");
    let listener = TcpListener::bind(config.listen).await.expect("listen");
    let address = listener.local_addr().expect("address");
    let (stop, receiver) = oneshot::channel();
    let running = server.clone();
    let task = tokio::spawn(orchester_server::serve(running, listener, async {
      // Dropping the fixture also requests shutdown.
      // Both an explicit signal and closing the fixture's sender request shutdown.
      receiver.await.unwrap_or(());
    }));
    Self {
      server,
      directory,
      clock,
      address,
      stop: Some(stop),
      task: Some(task),
    }
  }

  pub async fn finish(mut self) {
    self
      .stop
      .take()
      .expect("shutdown sender")
      .send(())
      .expect("server listening");
    tokio::time::timeout(Duration::from_secs(10), self.task.take().expect("server task"))
      .await
      .expect("shutdown deadline")
      .expect("join server")
      .expect("serve result");
  }

  pub async fn request(
    &self,
    method: &str,
    path: &str,
    token: Option<&Credential>,
    body: Option<serde_json::Value>,
  ) -> Response {
    self.request_headers(method, path, token, body, &[]).await
  }

  pub async fn request_headers(
    &self,
    method: &str,
    path: &str,
    token: Option<&Credential>,
    body: Option<serde_json::Value>,
    headers: &[(&str, &str)],
  ) -> Response {
    let stream = self.connect().await;
    exchange(stream, method, path, token, body, headers).await
  }

  pub async fn connect(&self) -> TcpStream {
    TcpStream::connect(self.address).await.expect("connect")
  }

  pub async fn tls_request(&self) -> Response {
    exchange(self.tls_connect().await, "GET", "/server", None, None, &[]).await
  }

  pub async fn tls_connect(&self) -> tokio_rustls::client::TlsStream<TcpStream> {
    use tokio_rustls::rustls::{
      self,
      pki_types::{CertificateDer, ServerName, pem::PemObject},
    };
    let mut roots = rustls::RootCertStore::empty();
    roots
      .add(CertificateDer::from_pem_slice(include_bytes!("../fixtures/ca.pem")).expect("test CA"))
      .expect("trust CA");
    let config = rustls::ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
      .with_safe_default_protocol_versions()
      .expect("protocols")
      .with_root_certificates(roots)
      .with_no_client_auth();
    let connector = tokio_rustls::TlsConnector::from(Arc::new(config));
    let tcp = self.connect().await;
    connector
      .connect(ServerName::try_from("localhost").expect("name"), tcp)
      .await
      .expect("TLS handshake")
  }

  pub async fn bootstrap(&self) -> SignedIn {
    let created = self
      .server
      .host_create_user(CreateUser {
        username: "alice".into(),
        display_name: "Alice".into(),
        admin: true,
      })
      .await
      .expect("bootstrap");
    let signed = self.login("alice", &created.initial_password).await;
    let password = credential("a secure admin password");
    let changed = self
      .request(
        "POST",
        "/auth/password",
        Some(&signed.token),
        Some(json!({"current_password": created.initial_password.expose(), "new_password": password.expose()})),
      )
      .await;
    assert_eq!(changed.status, 204);
    signed
  }

  pub async fn login(&self, username: &str, password: &Credential) -> SignedIn {
    let response = self
      .request(
        "POST",
        "/auth/sessions",
        None,
        Some(json!({"username": username, "password": password.expose(), "device": "test computer"})),
      )
      .await;
    assert_eq!(response.status, 201);
    response.json()
  }

  pub fn database(&self) -> rusqlite::Connection {
    rusqlite::Connection::open(self.directory.path().join("orchester-server.db")).expect("inspect database")
  }
}

impl Drop for TestServer {
  fn drop(&mut self) {
    // The graceful shutdown sender closes first; abort only this fixture's own task if the test panics.
    if let Some(task) = self.task.take() {
      task.abort();
    }
  }
}

pub fn credential(value: &str) -> Credential {
  Credential::new(value.into())
}

pub struct Response {
  pub status: u16,
  pub headers: HashMap<String, String>,
  body: Vec<u8>,
}

impl Response {
  pub fn json<T: DeserializeOwned>(&self) -> T {
    serde_json::from_slice(&self.body).unwrap_or_else(|_| panic!("response did not match JSON contract"))
  }

  pub fn contains(&self, value: &str) -> bool {
    String::from_utf8_lossy(&self.body).contains(value)
  }
}

async fn exchange(
  mut stream: impl AsyncRead + AsyncWrite + Unpin,
  method: &str,
  path: &str,
  token: Option<&Credential>,
  body: Option<serde_json::Value>,
  headers: &[(&str, &str)],
) -> Response {
  let body = body.map_or_else(Vec::new, |body| serde_json::to_vec(&body).expect("JSON request"));
  let mut request = format!(
    "{method} /api/v1{path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: {}\r\n",
    body.len()
  );
  if let Some(token) = token {
    request.push_str(&format!("Authorization: Bearer {}\r\n", token.expose()));
  }
  for (key, value) in headers {
    request.push_str(&format!("{key}: {value}\r\n"));
  }
  request.push_str("\r\n");
  stream.write_all(request.as_bytes()).await.expect("send headers");
  stream.write_all(&body).await.expect("send body");
  let mut response = Vec::new();
  tokio::time::timeout(Duration::from_secs(20), stream.read_to_end(&mut response))
    .await
    .expect("response deadline")
    .expect("read response");
  let boundary = response
    .windows(4)
    .position(|part| part == b"\r\n\r\n")
    .expect("HTTP response");
  let header = std::str::from_utf8(&response[..boundary]).expect("HTTP headers");
  let mut lines = header.split("\r\n");
  let status = lines
    .next()
    .expect("status")
    .split_whitespace()
    .nth(1)
    .expect("status code")
    .parse()
    .expect("numeric status");
  let headers = lines
    .map(|line| {
      let (key, value) = line.split_once(':').expect("header");
      (key.to_ascii_lowercase(), value.trim().to_owned())
    })
    .collect();
  Response {
    status,
    headers,
    body: response[boundary + 4..].to_vec(),
  }
}
