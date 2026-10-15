//! A real `orchester-server` on loopback with a temporary data directory, on a runtime of its own so
//! a test can drive the desktop (which has its own runtime) or call the service directly beside it.
//! People join it as API 3 has them join: the first admin with the setup code, the others with
//! invite codes; what the desktop never does (inviting, creating projects) is set up here through the
//! server's API, as the server admin would.

#![allow(dead_code, reason = "shared by several test binaries, each of which uses a part")]

use std::{net::SocketAddr, path::PathBuf, sync::Arc};

use orchester_api::{
  CodeJoin, Credential, SharedProject, UserChanges,
  client::{Client, PlainHttp, ServerAddress},
};
use orchester_core::secret::SecretValue;
use orchester_server::{
  Server,
  clock::SystemClock,
  config::{Config, TlsMode},
};
use parking_lot::Mutex;
use serde_json::{Value, json};
use tempfile::TempDir;
use tokio::{
  io::{AsyncReadExt, AsyncWriteExt},
  net::{TcpListener, TcpStream},
  runtime::Runtime,
  sync::oneshot,
};

/// The name the test server gives itself.
pub const SERVER_NAME: &str = "Kontur team";

pub struct TestServer {
  pub server: Server,
  pub address: SocketAddr,
  runtime: Runtime,
  stop: Option<oneshot::Sender<()>>,
  tls: bool,
  /// The first admin's username, once a code made them, and a device token of theirs for setting tests up.
  admin: Mutex<(Option<String>, Option<String>)>,
  _directory: TempDir,
}

impl TestServer {
  /// A server on plain HTTP.
  pub fn start() -> Self {
    Self::with(TlsMode::Off)
  }

  /// A server that presents its own generated certificate.
  pub fn start_tls() -> Self {
    Self::with(TlsMode::SelfSigned)
  }

  fn with(tls: TlsMode) -> Self {
    let runtime = Runtime::new().expect("server runtime");
    let scratch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.tmp/desktop-sharing/tests");
    std::fs::create_dir_all(&scratch).expect("scratch directory");
    let directory = tempfile::Builder::new()
      .prefix("server-")
      .tempdir_in(&scratch)
      .expect("temporary data");
    let (server, address, stop) = serve(&runtime, &directory, tls);
    Self {
      server,
      address,
      runtime,
      stop: Some(stop),
      tls: tls != TlsMode::Off,
      admin: Mutex::new((None, None)),
      _directory: directory,
    }
  }

  /// Starts the server again on the same data directory, at a new port, presenting its own generated certificate:
  /// what was set up over plain HTTP (accounts, the cloud link, invites) stays. Set-up requests speak plain HTTP, so
  /// a test sets up first.
  pub fn serve_self_signed(&mut self) {
    if let Some(stop) = self.stop.take() {
      // The server may have stopped already; a new one takes its place either way.
      let _ = stop.send(());
    }
    let (server, address, stop) = serve(&self.runtime, &self._directory, TlsMode::SelfSigned);
    (self.server, self.address, self.stop, self.tls) = (server, address, Some(stop), true);
  }

  /// The address as a person types it.
  pub fn url(&self) -> String {
    let scheme = if self.tls { "https" } else { "http" };
    format!("{scheme}://localhost:{}", self.address.port())
  }

  /// Runs `work` on the server's runtime.
  pub fn block_on<T>(&self, work: impl Future<Output = T>) -> T {
    self.runtime.block_on(work)
  }

  /// A code that makes the account `username`: the setup code for the first, which becomes the admin, and after it
  /// an invite code from that admin, as the server admin hands them out.
  pub fn code_for(&self, username: &str, admin: bool) -> Credential {
    let first = self.admin.lock().0.is_none();
    if first {
      self.admin.lock().0 = Some(username.to_owned());
      return self
        .block_on(self.server.host_setup_code())
        .expect("setup code")
        .expect("no admin yet");
    }
    let invite = self.admin_request(
      "POST",
      "/invites",
      &json!({"kind":"server","delivery":"code","admin":admin}),
    );
    Credential::new(invite["code"].as_str().expect("invite code").into())
  }

  /// The account `username` joined on a test computer; returns that device's token. API 3 has no passwords: the
  /// frozen `crates/desktop`'s tests use this name for what a first password was.
  pub fn create_user(&self, username: &str, display_name: &str, admin: bool) -> Credential {
    let code = self.code_for(username, admin);
    let client = self.client();
    let joined = self
      .block_on(client.join_with_code(&CodeJoin {
        code,
        device: "test computer".into(),
        username: Some(username.into()),
        display_name: Some(display_name.into()),
      }))
      .expect("join with a code");
    joined.token
  }

  /// Another device of `username`, joined with an account code from the host: that device's token.
  pub fn device_of(&self, username: &str) -> SecretValue {
    let code = self
      .block_on(self.server.host_account_code(username.into()))
      .expect("account code");
    let client = self.client();
    let joined = self
      .block_on(client.join_with_code(&CodeJoin {
        code,
        device: "BUILD-02 · Windows".into(),
        username: None,
        display_name: None,
      }))
      .expect("join with an account code");
    SecretValue::new(joined.token.expose().to_owned()).expect("a token")
  }

  /// Another computer of `username` (the passwords stand for nothing since API 3); returns its device token.
  pub fn sign_in_elsewhere(&self, username: &str, _first: &str, _own: &str) -> SecretValue {
    self.device_of(username)
  }

  /// A pairing code made on the device whose token is `token`, for the same person's next computer.
  pub fn pairing_code(&self, token: &SecretValue) -> Credential {
    let pairing = self.request("POST", "/auth/pairings", Some(token.expose()), &json!({}));
    Credential::new(pairing["code"].as_str().expect("pairing code").into())
  }

  /// The project `slug`, made empty by the first admin in the server admin, with `remote_hint` when given.
  pub fn create_project(&self, slug: &str, remote_hint: Option<&str>) -> SharedProject {
    let body = json!({"idempotency_key": orchester_core::id::IdempotencyKey::generate(), "name": "Kontur",
      "slug": slug, "key": "ORC", "remote_hint": remote_hint, "tasks": []});
    let created = self.admin_request("POST", "/projects", &body);
    serde_json::from_value(created["project"].clone()).expect("project")
  }

  /// The project `name` at `slug`, made empty by the first admin in the server admin, with `remote_hint` when given.
  pub fn create_named_project(
    &self,
    (name, key): (&str, &str),
    slug: &str,
    remote_hint: Option<&str>,
  ) -> SharedProject {
    let body = json!({"idempotency_key": orchester_core::id::IdempotencyKey::generate(), "name": name,
      "slug": slug, "key": key, "remote_hint": remote_hint, "tasks": []});
    let created = self.admin_request("POST", "/projects", &body);
    serde_json::from_value(created["project"].clone()).expect("project")
  }

  /// An invite code to `project` as a member, as an owner hands it out in the server admin.
  pub fn project_code(&self, project: &SharedProject) -> Credential {
    let invite = self.admin_request(
      "POST",
      "/invites",
      &json!({"kind":"project","delivery":"code","project":project.id,"role":"member"}),
    );
    Credential::new(invite["code"].as_str().expect("invite code").into())
  }

  /// Adds `username` to `project` as a member, as an owner does in the server admin.
  pub fn add_member(&self, project: &SharedProject, username: &str) {
    let path = format!("/projects/{}/members", project.id);
    self.admin_request("POST", &path, &json!({"username": username, "role": "member"}));
  }

  /// An invite to `project` as a member by email, which the cloud routes; returns its id.
  pub fn project_email_invite(&self, email: &str, project: &SharedProject) -> String {
    let invite = self.admin_request(
      "POST",
      "/invites",
      &json!({"kind":"project","delivery":"cloud","email":email,"project":project.id,"role":"member"}),
    );
    invite["invite"]["id"].as_str().expect("invite id").to_owned()
  }

  /// Links the server to an Orchester Cloud whose root key is `root_key`, with the key set it signed, as the first
  /// admin does at setup; returns the server's id, the audience of identity tokens for it.
  pub fn link_cloud(&self, root_key: &str, key_set: &str) -> String {
    let linked = self.admin_request(
      "PATCH",
      "/server/cloud",
      &json!({"link": {"root_key": root_key, "key_set": key_set}}),
    );
    linked["cloud"]["server"].as_str().expect("server id").to_owned()
  }

  /// An invite to the server by email, which the cloud routes; returns its id.
  pub fn email_invite(&self, email: &str) -> String {
    let invite = self.admin_request(
      "POST",
      "/invites",
      &json!({"kind":"server","delivery":"cloud","email":email}),
    );
    invite["invite"]["id"].as_str().expect("invite id").to_owned()
  }

  fn client(&self) -> Client {
    Client::new(
      ServerAddress::parse(&format!("http://localhost:{}", self.address.port())).expect("address"),
      PlainHttp::NotAllowed,
    )
    .expect("client")
  }

  /// Whether the server still accepts `token`.
  pub fn accepts(&self, token: &SecretValue) -> bool {
    let client = self.client();
    self.block_on(async { client.me(token).await.is_ok() })
  }

  /// An admin signs every device of `username` out (the frozen app's tests call this a password reset).
  pub fn reset_password(&self, admin: &SecretValue, username: &str) {
    let client = self.client();
    let users = self.block_on(client.users(admin)).expect("users");
    let user = users
      .into_iter()
      .find(|user| user.username == username)
      .expect("the user exists");
    let path = format!("/admin/users/{}/sessions", user.id);
    self.request("DELETE", &path, Some(admin.expose()), &Value::Null);
  }

  /// An admin's change of whether `username` is an admin of the server.
  pub fn set_admin(&self, admin: &SecretValue, username: &str, is_admin: bool) {
    let client = self.client();
    self.block_on(async {
      let users = client.users(admin).await.expect("users");
      let user = users
        .into_iter()
        .find(|user| user.username == username)
        .expect("the user exists");
      let changes = UserChanges {
        display_name: None,
        admin: Some(is_admin),
        disabled: None,
      };
      client.update_user(admin, user.id, &changes).await.expect("admin flag");
    });
  }

  /// A request of the first admin, from a device of theirs this support joins once.
  fn admin_request(&self, method: &str, path: &str, body: &Value) -> Value {
    let token = {
      let (username, token) = self.admin.lock().clone();
      match token {
        Some(token) => token,
        None => {
          let token = self.device_of(&username.expect("the first admin joined"));
          self.admin.lock().1 = Some(token.expose().to_owned());
          token.expose().to_owned()
        }
      }
    };
    self.request(method, path, Some(&token), body)
  }

  /// A request over plain HTTP to the server's API; panics unless it succeeds.
  fn request(&self, method: &str, path: &str, token: Option<&str>, body: &Value) -> Value {
    assert!(!self.tls, "set-up requests speak plain HTTP");
    let (status, answer) = self.block_on(exchange(self.address, method, path, token, body));
    assert!((200..300).contains(&status), "{method} {path}: {status}");
    answer
  }
}

/// The server on `directory`, serving on a free loopback port until the returned sender stops it.
fn serve(runtime: &Runtime, directory: &TempDir, tls: TlsMode) -> (Server, SocketAddr, oneshot::Sender<()>) {
  let config = Config {
    data_dir: directory.path().into(),
    listen: "127.0.0.1:0".parse().expect("loopback"),
    name: SERVER_NAME.into(),
    tls,
    ..Config::default()
  };
  let (server, listener) = runtime.block_on(async {
    let server = Server::open(config.clone(), Arc::new(SystemClock))
      .await
      .expect("open server");
    let listener = TcpListener::bind(config.listen).await.expect("listen");
    (server, listener)
  });
  let address = listener.local_addr().expect("address");
  let (stop, receiver) = oneshot::channel();
  runtime.spawn(orchester_server::serve(server.clone(), listener, async {
    // Both an explicit stop and dropping the fixture's sender shut the server down.
    receiver.await.unwrap_or(());
  }));
  (server, address, stop)
}

impl Drop for TestServer {
  fn drop(&mut self) {
    if let Some(stop) = self.stop.take() {
      // The server may have stopped already; its runtime ends with the fixture either way.
      let _ = stop.send(());
    }
  }
}

async fn exchange(address: SocketAddr, method: &str, path: &str, token: Option<&str>, body: &Value) -> (u16, Value) {
  let body = if body.is_null() {
    String::new()
  } else {
    body.to_string()
  };
  let mut request = format!(
    "{method} /api/v1{path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: {}\r\n",
    body.len()
  );
  if let Some(token) = token {
    request.push_str(&format!("Authorization: Bearer {token}\r\n"));
  }
  request.push_str("\r\n");
  request.push_str(&body);
  let mut stream = TcpStream::connect(address).await.expect("connect");
  stream.write_all(request.as_bytes()).await.expect("send");
  let mut response = Vec::new();
  stream.read_to_end(&mut response).await.expect("read");
  let text = String::from_utf8(response).expect("UTF-8 answer");
  let (head, answer) = text.split_once("\r\n\r\n").expect("HTTP response");
  let status = head
    .split_whitespace()
    .nth(1)
    .and_then(|status| status.parse().ok())
    .expect("status");
  (status, serde_json::from_str(answer).unwrap_or(Value::Null))
}
