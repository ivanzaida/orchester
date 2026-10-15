//! A real `orchester-server` on loopback with a temporary data directory.

#![allow(dead_code, reason = "shared by several test binaries, each of which uses a part")]

use std::{net::SocketAddr, path::PathBuf, sync::Arc, time::Duration};

use orchester_api::{
  CodeJoin, Credential, DeviceKind, Joined,
  client::{Client, PlainHttp, ServerAddress},
};
use orchester_core::{id::UserId, secret::SecretValue};
use orchester_server::{
  Server,
  clock::SystemClock,
  config::{Config, TlsMode},
  error::ServerError,
};
use serde_json::{Value, json};
use tempfile::TempDir;
use tokio::{net::TcpListener, sync::oneshot, task::JoinHandle};

pub struct TestServer {
  pub server: Server,
  pub address: SocketAddr,
  pub directory: TempDir,
  stop: Option<oneshot::Sender<()>>,
  task: Option<JoinHandle<Result<(), ServerError>>>,
}

impl TestServer {
  /// A server on plain HTTP unless `configure` asks for TLS.
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
      tls: TlsMode::Off,
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
      directory,
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

  /// Joins with `code` from this computer; `username` when the code makes an account.
  pub async fn join(&self, code: &Credential, username: Option<&str>) -> Joined {
    self
      .client()
      .join_with_code(&CodeJoin {
        code: Credential::new(code.expose().into()),
        device: "test computer · Windows".into(),
        username: username.map(Into::into),
        display_name: username.map(Into::into),
        kind: DeviceKind::App,
      })
      .await
      .expect("join with a code")
  }

  /// The first admin, `alice`, from the setup code; returns the device token.
  pub async fn admin(&self) -> SecretValue {
    let code = self
      .server
      .host_setup_code()
      .await
      .expect("setup code")
      .expect("no admin yet");
    token(&self.join(&code, Some("alice")).await.token)
  }

  /// `username`, invited to the server by `admin` with a code and joined: their id and token.
  pub async fn person(&self, admin: &SecretValue, username: &str) -> (UserId, SecretValue) {
    let invite = self
      .post(admin, "/invites", json!({"kind":"server","delivery":"code"}))
      .await;
    let code = Credential::new(invite["code"].as_str().expect("invite code").into());
    let joined = self.join(&code, Some(username)).await;
    (joined.user.id, token(&joined.token))
  }

  /// A request the desktop never makes (admin actions belong to the server admin), sent here to set a test up.
  pub async fn post(&self, token: &SecretValue, path: &str, body: Value) -> Value {
    // reqwest needs the process's crypto provider even for plain HTTP; an installed one serves as well.
    let _ = rustls::crypto::ring::default_provider().install_default();
    let response = reqwest::Client::builder()
      .no_proxy()
      .build()
      .expect("setup client")
      .post(format!("{}/api/v1{path}", self.url()))
      .header("Authorization", format!("Bearer {}", token.expose()))
      .header("Content-Type", "application/json")
      .body(body.to_string())
      .send()
      .await
      .expect("setup request");
    assert!(response.status().is_success(), "{path}: {}", response.status());
    serde_json::from_slice(&response.bytes().await.expect("setup answer")).expect("setup JSON")
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

pub fn token(credential: &Credential) -> SecretValue {
  SecretValue::new(credential.expose().to_owned()).expect("a token is not blank")
}
