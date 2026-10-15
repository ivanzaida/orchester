//! A real `orchester-server` on loopback with a temporary data directory, on a runtime of its own so
//! a test can drive the desktop (which has its own runtime) or call the service directly beside it.

#![allow(dead_code, reason = "shared by several test binaries, each of which uses a part")]

use std::{net::SocketAddr, path::PathBuf, sync::Arc};

use orchester_api::{
  CodeJoin, Credential, PasswordChange, SignIn, UserChanges,
  client::{Client, PlainHttp, ServerAddress},
};
use orchester_core::secret::SecretValue;
use orchester_server::{
  Server,
  clock::SystemClock,
  config::{Config, TlsMode},
};
use parking_lot::Mutex;
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
  /// The first account's device token, which invites the later ones.
  admin: Mutex<Option<String>>,
  _directory: TempDir,
}

impl TestServer {
  pub fn start() -> Self {
    let runtime = Runtime::new().expect("server runtime");
    let scratch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.tmp/desktop-sharing/tests");
    std::fs::create_dir_all(&scratch).expect("scratch directory");
    let directory = tempfile::Builder::new()
      .prefix("server-")
      .tempdir_in(&scratch)
      .expect("temporary data");
    let config = Config {
      data_dir: directory.path().into(),
      listen: "127.0.0.1:0".parse().expect("loopback"),
      name: SERVER_NAME.into(),
      tls: TlsMode::Off,
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
    Self {
      server,
      address,
      runtime,
      stop: Some(stop),
      admin: Mutex::new(None),
      _directory: directory,
    }
  }

  /// The address as a person types it.
  pub fn url(&self) -> String {
    format!("http://localhost:{}", self.address.port())
  }

  /// Runs `work` on the server's runtime.
  pub fn block_on<T>(&self, work: impl Future<Output = T>) -> T {
    self.runtime.block_on(work)
  }

  /// An account joined with a code: the first with the setup code (an admin), later ones with an invite code the
  /// first account makes. Returns the account's device token. API 3 has no passwords, so the tests that sign in
  /// with a password fail until the workspace joins with codes and cloud identities (S1c's workspace part).
  pub fn create_user(&self, username: &str, display_name: &str, admin: bool) -> Credential {
    let first = self.admin.lock().clone();
    let code = match first {
      None => self
        .block_on(self.server.host_setup_code())
        .expect("setup code")
        .expect("no admin yet"),
      Some(token) => {
        let body = format!(r#"{{"kind":"server","delivery":"code","admin":{admin}}}"#);
        let invite = self.block_on(post(self.address, &token, "/invites", &body));
        Credential::new(invite["code"].as_str().expect("invite code").into())
      }
    };
    let client = self.client();
    let joined = self
      .block_on(client.join_with_code(&CodeJoin {
        code,
        device: "test computer".into(),
        username: Some(username.into()),
        display_name: Some(display_name.into()),
      }))
      .expect("join with a code");
    self
      .admin
      .lock()
      .get_or_insert_with(|| joined.token.expose().to_owned());
    joined.token
  }

  fn client(&self) -> Client {
    Client::new(
      ServerAddress::parse(&self.url()).expect("address"),
      PlainHttp::NotAllowed,
    )
    .expect("client")
  }

  /// Signs `username` in from another computer, choosing `own` as the password when `first` is a
  /// first password; returns that session's token.
  pub fn sign_in_elsewhere(&self, username: &str, first: &str, own: &str) -> SecretValue {
    let client = self.client();
    self.block_on(async {
      let signed = client
        .sign_in(&SignIn {
          username: username.into(),
          password: Credential::new(first.into()),
          device: "BUILD-02 · Windows".into(),
        })
        .await
        .expect("sign in elsewhere");
      let token = SecretValue::new(signed.token.expose().to_owned()).expect("a token");
      if signed.password_change_required {
        client
          .change_password(
            &token,
            &PasswordChange {
              current_password: Credential::new(first.into()),
              new_password: Credential::new(own.into()),
            },
          )
          .await
          .expect("choose a password");
      }
      token
    })
  }

  /// Whether the server still accepts `token`.
  pub fn accepts(&self, token: &SecretValue) -> bool {
    let client = self.client();
    self.block_on(async { client.me(token).await.is_ok() })
  }

  /// An admin's reset of `username`'s password, which signs every session of theirs out.
  pub fn reset_password(&self, admin: &SecretValue, username: &str) {
    let client = self.client();
    self.block_on(async {
      let users = client.users(admin).await.expect("users");
      let user = users
        .into_iter()
        .find(|user| user.username == username)
        .expect("the user exists");
      client.reset_password(admin, user.id).await.expect("reset");
    });
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
}

impl Drop for TestServer {
  fn drop(&mut self) {
    if let Some(stop) = self.stop.take() {
      // The server may have stopped already; its runtime ends with the fixture either way.
      let _ = stop.send(());
    }
  }
}

/// A request the desktop never makes (an admin action of the server admin), sent to set a test up.
async fn post(address: SocketAddr, token: &str, path: &str, body: &str) -> serde_json::Value {
  let mut stream = TcpStream::connect(address).await.expect("connect");
  let request = format!(
    "POST /api/v1{path} HTTP/1.1
Host: localhost
Connection: close
Authorization: Bearer {token}
Content-Type: application/json
Content-Length: {}

{body}",
    body.len()
  );
  stream.write_all(request.as_bytes()).await.expect("send");
  let mut response = Vec::new();
  stream.read_to_end(&mut response).await.expect("read");
  let start = response
    .windows(4)
    .position(|part| {
      part
        == b"

"
    })
    .expect("HTTP response")
    + 4;
  serde_json::from_slice(&response[start..]).expect("JSON answer")
}
