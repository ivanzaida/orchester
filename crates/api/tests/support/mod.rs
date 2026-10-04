//! A real `orchester-server` on loopback with a temporary data directory.

use std::{net::SocketAddr, path::PathBuf, sync::Arc, time::Duration};

use orchester_api::{
  CreateUser, Credential, PasswordChange, SignIn,
  client::{Client, PlainHttp, ServerAddress},
};
use orchester_core::secret::SecretValue;
use orchester_server::{Server, clock::SystemClock, config::Config, error::ServerError};
use tempfile::TempDir;
use tokio::{net::TcpListener, sync::oneshot, task::JoinHandle};

/// The admin's own password after the first sign-in.
pub const ADMIN_PASSWORD: &str = "a secure admin password";

pub struct TestServer {
  pub server: Server,
  pub address: SocketAddr,
  _directory: TempDir,
  stop: Option<oneshot::Sender<()>>,
  task: Option<JoinHandle<Result<(), ServerError>>>,
}

impl TestServer {
  pub async fn start(configure: impl FnOnce(&mut Config)) -> Self {
    let scratch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.tmp/api-client/tests");
    std::fs::create_dir_all(&scratch).expect("scratch directory");
    let directory = tempfile::Builder::new()
      .prefix("client-")
      .tempdir_in(&scratch)
      .expect("temporary data");
    let mut config = Config {
      data_dir: directory.path().into(),
      listen: "127.0.0.1:0".parse().expect("loopback"),
      name: "Kontur team".into(),
      ..Config::default()
    };
    configure(&mut config);
    let server = Server::open(config.clone(), Arc::new(SystemClock))
      .await
      .expect("open server");
    let listener = TcpListener::bind(config.listen).await.expect("listen");
    let address = listener.local_addr().expect("address");
    let (stop, receiver) = oneshot::channel();
    let task = tokio::spawn(orchester_server::serve(server.clone(), listener, async {
      // Both an explicit stop and dropping the fixture's sender shut the server down.
      receiver.await.unwrap_or(());
    }));
    Self {
      server,
      address,
      _directory: directory,
      stop: Some(stop),
      task: Some(task),
    }
  }

  /// The server's address as a person types it.
  pub fn url(&self) -> String {
    format!("http://localhost:{}", self.address.port())
  }

  pub fn client(&self) -> Client {
    Client::new(
      ServerAddress::parse(&self.url()).expect("address"),
      PlainHttp::NotAllowed,
    )
    .expect("client")
  }

  /// Creates `username` on the host, as `orchester-server user create` does, and returns the first
  /// password.
  pub async fn create_user(&self, username: &str, admin: bool) -> Credential {
    self
      .server
      .host_create_user(CreateUser {
        username: username.into(),
        display_name: username.into(),
        admin,
      })
      .await
      .expect("create user on the host")
      .initial_password
  }

  /// The first admin, signed in with a password of their own; returns the token.
  pub async fn admin(&self) -> SecretValue {
    let first = self.create_user("alice", true).await;
    let client = self.client();
    let signed = client
      .sign_in(&sign_in("alice", first.expose()))
      .await
      .expect("first sign-in");
    let token = token(&signed.token);
    client
      .change_password(
        &token,
        &PasswordChange {
          current_password: Credential::new(first.expose().into()),
          new_password: Credential::new(ADMIN_PASSWORD.into()),
        },
      )
      .await
      .expect("choose a password");
    token
  }

  pub async fn finish(mut self) {
    self
      .stop
      .take()
      .expect("stop sender")
      .send(())
      .expect("server listening");
    tokio::time::timeout(Duration::from_secs(10), self.task.take().expect("server task"))
      .await
      .expect("shutdown deadline")
      .expect("join server")
      .expect("serve result");
  }
}

impl Drop for TestServer {
  fn drop(&mut self) {
    if let Some(task) = self.task.take() {
      task.abort();
    }
  }
}

pub fn sign_in(username: &str, password: &str) -> SignIn {
  SignIn {
    username: username.into(),
    password: Credential::new(password.into()),
    device: "test computer · Windows".into(),
  }
}

pub fn token(credential: &Credential) -> SecretValue {
  SecretValue::new(credential.expose().to_owned()).expect("a token is not blank")
}
