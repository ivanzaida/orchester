use std::{
  collections::HashMap,
  net::{IpAddr, SocketAddr},
  sync::{
    Arc,
    atomic::{AtomicI64, Ordering},
  },
  time::Duration,
};

use orchester_api::{Credential, Joined, PairingCode};
use orchester_server::{
  Server,
  clock::Clock,
  config::{Config, TlsMode},
};
use parking_lot::Mutex;
use serde::de::DeserializeOwned;
use serde_json::json;
use tempfile::TempDir;
use tokio::{
  io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
  net::{TcpListener, TcpSocket, TcpStream},
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

pub mod admin_app;
pub mod cloud;
mod pinned;

pub struct TestServer {
  pub server: Server,
  pub directory: TempDir,
  pub clock: Arc<TestClock>,
  /// The first admin's device token, once `bootstrap` made them; `user` invites people with it.
  admin: Mutex<Option<String>>,
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
    // Plain HTTP unless a test asks for TLS: most tests speak HTTP to the server directly.
    let mut config = Config {
      data_dir: directory.path().into(),
      listen: "127.0.0.1:0".parse().expect("loopback"),
      tls: TlsMode::Off,
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
      admin: Mutex::new(None),
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

  /// A request outside the API (`path` as sent), as a browser asks for the server admin's pages.
  pub async fn page(&self, method: &str, path: &str, headers: &[(&str, &str)]) -> Response {
    exchange_at(self.connect().await, method, path, None, None, headers).await
  }

  /// Starts the private listener the server admin app calls the API on, and returns its address.
  pub async fn start_admin_api(&self) -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("listen");
    let address = listener.local_addr().expect("address");
    tokio::spawn(orchester_server::serve_admin_api(
      self.server.clone(),
      listener,
      std::future::pending(),
    ));
    address
  }

  pub async fn connect(&self) -> TcpStream {
    TcpStream::connect(self.address).await.expect("connect")
  }

  pub async fn connect_from(&self, source: IpAddr) -> TcpStream {
    let socket = TcpSocket::new_v4().expect("IPv4 test socket");
    socket.bind(SocketAddr::new(source, 0)).expect("bind source address");
    socket.connect(self.address).await.expect("connect from source")
  }

  pub async fn request_from(&self, source: IpAddr, tls: bool) -> Response {
    let stream = self.connect_from(source).await;
    if tls {
      exchange(self.tls_on(stream).await, "GET", "/server", None, None, &[]).await
    } else {
      exchange(stream, "GET", "/server", None, None, &[]).await
    }
  }

  pub async fn tls_request(&self) -> Response {
    exchange(self.tls_connect().await, "GET", "/server", None, None, &[]).await
  }

  /// A request over TLS that trusts exactly the server's own certificate, as a pinned desktop does.
  pub async fn tls_pinned_request(
    &self,
    method: &str,
    path: &str,
    token: Option<&Credential>,
    body: Option<serde_json::Value>,
  ) -> Response {
    use tokio_rustls::rustls::{self, pki_types::ServerName};
    let pin = self
      .server
      .check_code()
      .expect("the server presents its own certificate");
    let config = rustls::ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
      .with_safe_default_protocol_versions()
      .expect("protocols")
      .dangerous()
      .with_custom_certificate_verifier(Arc::new(pinned::Pinned::new(pin)))
      .with_no_client_auth();
    let connector = tokio_rustls::TlsConnector::from(Arc::new(config));
    let stream = connector
      .connect(ServerName::try_from("localhost").expect("name"), self.connect().await)
      .await
      .expect("TLS handshake with the pinned certificate");
    exchange(stream, method, path, token, body, &[]).await
  }

  pub async fn tls_connect(&self) -> tokio_rustls::client::TlsStream<TcpStream> {
    self.tls_on(self.connect().await).await
  }

  pub async fn tls_on(&self, tcp: TcpStream) -> tokio_rustls::client::TlsStream<TcpStream> {
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
    connector
      .connect(ServerName::try_from("localhost").expect("name"), tcp)
      .await
      .expect("TLS handshake")
  }

  /// The first admin, `alice`, from the setup code the server prints while it has no admin.
  pub async fn bootstrap(&self) -> Joined {
    let code = self
      .server
      .host_setup_code()
      .await
      .expect("setup code")
      .expect("no admin yet");
    let joined = self.join(&code, Some("alice"), "test computer").await;
    *self.admin.lock() = Some(joined.token.expose().to_owned());
    joined
  }

  /// Joins with `code`; `username` (also the display name) when the code makes an account.
  pub async fn join(&self, code: &Credential, username: Option<&str>, device: &str) -> Joined {
    let response = self.redeem(code, username, device).await;
    assert_eq!(response.status, 201);
    response.json()
  }

  pub async fn redeem(&self, code: &Credential, username: Option<&str>, device: &str) -> Response {
    let mut body = json!({"code": code.expose(), "device": device});
    if let Some(username) = username {
      body["username"] = json!(username);
      let mut display = username.to_owned();
      display[..1].make_ascii_uppercase();
      body["display_name"] = json!(display);
    }
    self.request("POST", "/auth/code", None, Some(body)).await
  }

  /// A person invited to the server by the first admin with a code, joined on their first device.
  pub async fn invited(&self, username: &str, admin: bool) -> Joined {
    let token = credential(self.admin.lock().as_deref().expect("bootstrap first"));
    let response = self
      .request(
        "POST",
        "/invites",
        Some(&token),
        Some(json!({"kind":"server","delivery":"code","admin":admin})),
      )
      .await;
    assert_eq!(response.status, 201);
    let code = credential(
      response.json::<serde_json::Value>()["code"]
        .as_str()
        .expect("invite code"),
    );
    self.join(&code, Some(username), "test computer").await
  }

  /// Another device of `token`'s account, joined with a pairing code.
  pub async fn another_device(&self, token: &Credential) -> Joined {
    let response = self.request("POST", "/auth/pairings", Some(token), None).await;
    assert_eq!(response.status, 201);
    let pairing: PairingCode = response.json();
    self.join(&pairing.code, None, "second computer").await
  }

  /// The server's time in seconds, as the cloud's tokens and key sets count it.
  pub fn seconds(&self) -> i64 {
    self.clock.now_ms() / 1000
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

/// An API request to the private listener at `address`, as the admin app sends it.
pub async fn admin_api(
  address: SocketAddr,
  method: &str,
  path: &str,
  token: Option<&Credential>,
  body: Option<serde_json::Value>,
  headers: &[(&str, &str)],
) -> Response {
  let stream = TcpStream::connect(address).await.expect("connect");
  exchange(stream, method, path, token, body, headers).await
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

  pub fn text(&self) -> String {
    String::from_utf8_lossy(&self.body).into_owned()
  }

  pub fn contains(&self, value: &str) -> bool {
    String::from_utf8_lossy(&self.body).contains(value)
  }
}

async fn exchange(
  stream: impl AsyncRead + AsyncWrite + Unpin,
  method: &str,
  path: &str,
  token: Option<&Credential>,
  body: Option<serde_json::Value>,
  headers: &[(&str, &str)],
) -> Response {
  exchange_at(stream, method, &format!("/api/v1{path}"), token, body, headers).await
}

async fn exchange_at(
  mut stream: impl AsyncRead + AsyncWrite + Unpin,
  method: &str,
  path: &str,
  token: Option<&Credential>,
  body: Option<serde_json::Value>,
  headers: &[(&str, &str)],
) -> Response {
  let body = body.map_or_else(Vec::new, |body| serde_json::to_vec(&body).expect("JSON request"));
  let mut request = format!(
    "{method} {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: {}\r\n",
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
