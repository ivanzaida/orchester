//! A real `orchester-server` on loopback with a temporary data directory, on a runtime of its own so
//! a test can drive the desktop (which has its own runtime) or call the service directly beside it.
//! People join it as API 3 has them join: the first admin with the setup code, the others with
//! invite codes; what the desktop never does (inviting, creating projects) is set up here through the
//! server's API, as the server admin would.

#![allow(dead_code, reason = "shared by several test binaries, each of which uses a part")]

mod joins;
#[allow(
  clippy::duplicate_mod,
  reason = "the tests' one cloud key, loaded by every support module that signs for it, whichever of them a test binary includes"
)]
pub mod test_cloud;
mod tls;

use std::{net::SocketAddr, path::PathBuf, sync::Arc};

use orchester_api::{
  Credential, SharedProject, UserChanges,
  certificate::CertificatePin,
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
  /// The certificate a server that presents its own from the start serves, which its set-up requests trust.
  pin: Option<CertificatePin>,
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

  /// A server that presents its own generated certificate from the start, at the address it names in its link
  /// statement (`https://localhost:<port>`), so Orchester Cloud's registry holds that address and certificate. Its
  /// set-up requests trust exactly that certificate.
  pub fn start_tls_linkable() -> Self {
    let port = std::net::TcpListener::bind("127.0.0.1:0")
      .and_then(|listener| listener.local_addr())
      .expect("a free port")
      .port();
    let mut server = Self::open(TlsMode::SelfSigned, Some(port));
    let address = ServerAddress::parse(&server.url()).expect("address");
    server.pin = Some(
      server
        .block_on(Client::presented_certificate(&address))
        .expect("the server's certificate"),
    );
    server
  }

  /// A server on plain HTTP that names `https://localhost:<its port>` as its address, as one behind a proxy that
  /// announces another address than the one a computer reaches it by: the project addresses it reports are on another
  /// server than the one that computer connected to.
  pub fn start_announcing_another_address() -> Self {
    let port = std::net::TcpListener::bind("127.0.0.1:0")
      .and_then(|listener| listener.local_addr())
      .expect("a free port")
      .port();
    Self::open(TlsMode::Off, Some(port))
  }

  fn with(tls: TlsMode) -> Self {
    Self::open(tls, None)
  }

  /// `port`: a fixed port, which the server also names as its address.
  fn open(tls: TlsMode, port: Option<u16>) -> Self {
    let runtime = Runtime::new().expect("server runtime");
    let scratch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.tmp/desktop-sharing/tests");
    std::fs::create_dir_all(&scratch).expect("scratch directory");
    let directory = tempfile::Builder::new()
      .prefix("server-")
      .tempdir_in(&scratch)
      .expect("temporary data");
    let (server, address, stop) = serve(&runtime, &directory, (tls, port));
    Self {
      server,
      address,
      runtime,
      stop: Some(stop),
      tls: tls != TlsMode::Off,
      pin: None,
      admin: Mutex::new((None, None)),
      _directory: directory,
    }
  }

  /// Starts the server again on the same data directory, at a new port, presenting its own generated certificate:
  /// what was set up over plain HTTP (accounts, the cloud link, invites) stays. Set-up requests from then on trust
  /// exactly that certificate (N1 design 3: `n1_backends` sets up after the switch).
  pub fn serve_self_signed(&mut self) {
    if let Some(stop) = self.stop.take() {
      // The server may have stopped already; a new one takes its place either way.
      let _ = stop.send(());
    }
    let (server, address, stop) = serve(&self.runtime, &self._directory, (TlsMode::SelfSigned, None));
    (self.server, self.address, self.stop, self.tls) = (server, address, Some(stop), true);
    let address = ServerAddress::parse(&self.url()).expect("address");
    self.pin = Some(
      self
        .block_on(Client::presented_certificate(&address))
        .expect("the server's certificate"),
    );
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

  /// Gives `username` the `role` (`owner` or `member`) in `project`, as an owner does in the server admin.
  pub fn set_role(&self, project: &SharedProject, username: &str, role: &str) {
    let path = format!("/projects/{}/members", project.id);
    let members = self.admin_request("GET", &path, &Value::Null);
    let user = members["members"]
      .as_array()
      .expect("members")
      .iter()
      .find(|member| member["username"] == username)
      .expect("a member")["user"]["id"]
      .as_str()
      .expect("member id")
      .to_owned();
    self.admin_request("PATCH", &format!("{path}/{user}"), &json!({ "role": role }));
  }

  /// Removes `username` from `project`, as an owner does in the server admin.
  pub fn remove_member(&self, project: &SharedProject, username: &str) {
    let path = format!("/projects/{}/members", project.id);
    let members = self.admin_request("GET", &path, &Value::Null);
    let user = members["members"]
      .as_array()
      .expect("members")
      .iter()
      .find(|member| member["username"] == username)
      .expect("a member")["user"]["id"]
      .as_str()
      .expect("member id")
      .to_owned();
    self.admin_request("DELETE", &format!("{path}/{user}"), &Value::Null);
  }

  /// The keys of `project`'s tasks on the server, by number, as the first admin reads them.
  pub fn task_keys(&self, project: &SharedProject) -> Vec<String> {
    let tasks = self.admin_request("GET", &format!("/projects/{}/tasks", project.id), &Value::Null);
    let mut keys: Vec<(u64, String)> = tasks["tasks"]
      .as_array()
      .expect("tasks")
      .iter()
      .map(|task| {
        (
          task["number"].as_u64().expect("number"),
          task["key"].as_str().expect("key").to_owned(),
        )
      })
      .collect();
    keys.sort();
    keys.into_iter().map(|(_, key)| key).collect()
  }

  /// Adds tasks of `contents` (the API's `TaskContent`) to `project`, as the first admin creates them from their
  /// computer.
  pub fn add_tasks(&self, project: &SharedProject, contents: &[Value]) {
    let body = json!({ "idempotency_key": orchester_core::id::IdempotencyKey::generate(), "tasks": contents });
    self.admin_request("POST", &format!("/projects/{}/tasks", project.id), &body);
  }

  /// Gives the task `key` of `project` the title `title`, as the first admin saves it from their computer.
  pub fn retitle(&self, project: &SharedProject, key: &str, title: &str) {
    self.change_task(project, key, &json!({ "title": title }));
  }

  /// Changes the task `key` of `project` by `changes` (the API's `Changes`), as the first admin saves it from their
  /// computer.
  pub fn change_task(&self, project: &SharedProject, key: &str, changes: &Value) {
    let path = format!("/projects/{}/tasks", project.id);
    let tasks = self.admin_request("GET", &path, &Value::Null);
    let task = tasks["tasks"]
      .as_array()
      .expect("tasks")
      .iter()
      .find(|task| task["key"] == key)
      .expect("the task")
      .clone();
    let body = json!({ "revision": task["revision"], "changes": changes });
    let id = task["id"].as_str().expect("task id");
    self.admin_request("PATCH", &format!("{path}/{id}"), &body);
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

  /// The link statement the server signs for Orchester Cloud (`link: "start"`), which the cloud registers it from.
  pub fn cloud_statement(&self) -> String {
    let started = self.admin_request("PATCH", "/server/cloud", &json!({"link": "start"}));
    started["statement"].as_str().expect("statement").to_owned()
  }

  /// Finishes the link with what Orchester Cloud answered the statement: its root key and key set, and the identity
  /// token that links the first admin's account to the cloud account that registered the server.
  pub fn finish_cloud_link(&self, root_key: &str, key_set: &str, identity_token: &str) {
    let link = json!({"link": {"root_key": root_key, "key_set": key_set, "identity_token": identity_token}});
    self.admin_request("PATCH", "/server/cloud", &link);
  }

  /// An invite by email to `project` as a member, or to the server alone: its id, and the invite the server signed
  /// for the cloud to route.
  pub fn signed_email_invite(&self, email: &str, project: Option<&SharedProject>) -> (String, String) {
    let body = match project {
      Some(project) => {
        json!({"kind":"project","delivery":"cloud","email":email,"project":project.id,"role":"member"})
      }
      None => json!({"kind":"server","delivery":"cloud","email":email}),
    };
    let invite = self.admin_request("POST", "/invites", &body);
    (
      invite["invite"]["id"].as_str().expect("invite id").to_owned(),
      invite["signed"].as_str().expect("signed invite").to_owned(),
    )
  }

  /// Turns join approval on or off: a cloud join of an account the server knows then waits for one of that account's
  /// computers, or an admin, to approve it.
  pub fn set_join_approval(&self, on: bool) {
    self.admin_request("PATCH", "/server/settings", &json!({ "join_approval": on }));
  }

  /// The first admin approves every join that waits for approval; answers how many.
  pub fn approve_waiting(&self) -> usize {
    self.answer_waiting(true)
  }

  /// The first admin refuses every join that waits for approval; answers how many.
  pub fn refuse_waiting(&self) -> usize {
    self.answer_waiting(false)
  }

  fn answer_waiting(&self, approve: bool) -> usize {
    let waiting = self.admin_request("GET", "/auth/approvals", &Value::Null);
    let ids: Vec<String> = waiting["approvals"]
      .as_array()
      .expect("approvals")
      .iter()
      .filter_map(|approval| approval["id"].as_str().map(str::to_owned))
      .collect();
    for id in &ids {
      self.admin_request("POST", &format!("/auth/approvals/{id}"), &json!({ "approve": approve }));
    }
    ids.len()
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

  pub fn client(&self) -> Client {
    if let Some(pin) = self.pin {
      return Client::pinned(ServerAddress::parse(&self.url()).expect("address"), pin).expect("client");
    }
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
          // Joined with an account code and a two-factor identity ([`Self::code_proof`]), as inviting needs.
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
    let (status, answer) = match self.pin {
      Some(pin) => self.block_on(async {
        let stream = tls::connect(self.address, pin).await;
        exchange_on(stream, method, path, token, body).await
      }),
      None => {
        assert!(!self.tls, "set-up requests speak plain HTTP");
        self.block_on(exchange(self.address, method, path, token, body))
      }
    };
    assert!((200..300).contains(&status), "{method} {path}: {status}");
    answer
  }
}

/// The server on `directory`, serving on a free loopback port until the returned sender stops it.
fn serve(
  runtime: &Runtime,
  directory: &TempDir,
  (tls, port): (TlsMode, Option<u16>),
) -> (Server, SocketAddr, oneshot::Sender<()>) {
  let config = Config {
    data_dir: directory.path().into(),
    listen: SocketAddr::from(([127, 0, 0, 1], port.unwrap_or(0))),
    name: SERVER_NAME.into(),
    tls,
    address: port.map(|port| format!("https://localhost:{port}")),
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
  let stream = TcpStream::connect(address).await.expect("connect");
  exchange_on(stream, method, path, token, body).await
}

/// A request on `stream`, its status and its JSON answer.
async fn exchange_on(
  mut stream: impl tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
  method: &str,
  path: &str,
  token: Option<&str>,
  body: &Value,
) -> (u16, Value) {
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
  stream.write_all(request.as_bytes()).await.expect("send");
  let mut response = Vec::new();
  // A TLS server may close without a close_notify; what was read is the whole answer then.
  if let Err(error) = stream.read_to_end(&mut response).await {
    assert!(!response.is_empty(), "read: {error}");
  }
  let text = String::from_utf8(response).expect("UTF-8 answer");
  let (head, answer) = text.split_once("\r\n\r\n").expect("HTTP response");
  let status = head
    .split_whitespace()
    .nth(1)
    .and_then(|status| status.parse().ok())
    .expect("status");
  (status, serde_json::from_str(answer).unwrap_or(Value::Null))
}
