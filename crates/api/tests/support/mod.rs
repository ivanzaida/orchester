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
  host::CloudCommand,
};
use serde_json::{Value, json};
use tempfile::TempDir;
use tokio::{net::TcpListener, sync::oneshot, task::JoinHandle};

#[path = "../../../workspace/tests/server_support/test_cloud.rs"]
mod test_cloud;

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

  /// Joins with `code`, which makes the account `username`, from this computer, with that person's identity token for
  /// codes, which every code but the setup code needs ([`Self::code_proof`]).
  pub async fn join(&self, code: &Credential, username: &str) -> Joined {
    let (identity_token, key_set) = self.code_proof(username).await;
    self
      .join_with(code, Some(username), (Some(identity_token), Some(key_set)))
      .await
  }

  /// Joins with an account or pairing `code` of `person`'s account, with `person`'s identity token for codes.
  pub async fn join_as(&self, code: &Credential, person: &str) -> Joined {
    let (identity_token, key_set) = self.code_proof(person).await;
    self.join_with(code, None, (Some(identity_token), Some(key_set))).await
  }

  async fn join_with(
    &self,
    code: &Credential,
    username: Option<&str>,
    (identity_token, key_set): (Option<Credential>, Option<String>),
  ) -> Joined {
    self
      .client()
      .join_with_code(&CodeJoin {
        code: Credential::new(code.expose().into()),
        device: "test computer · Windows".into(),
        username: username.map(Into::into),
        display_name: username.map(Into::into),
        kind: DeviceKind::App,
        identity_token,
        key_set,
      })
      .await
      .expect("join with a code")
  }

  /// What the app of `person`, signed in to Orchester Cloud with two-factor authentication, sends with a code: that
  /// person's identity token for codes to this server from the tests' cloud, and its key set. The server pins that
  /// cloud's root first, as its admin does on the host.
  pub async fn code_proof(&self, person: &str) -> (Credential, String) {
    let shown = self
      .server
      .host_cloud(CloudCommand::Show)
      .await
      .expect("the cloud link");
    let link = match shown.root_fingerprint {
      Some(_) => shown,
      None => self
        .server
        .host_cloud(CloudCommand::Pin(test_cloud::root_key()))
        .await
        .expect("pin the tests' cloud"),
    };
    let token = test_cloud::code_token(&link.server, person);
    (Credential::new(token), test_cloud::key_set(test_cloud::now()))
  }

  /// The first admin, `alice`, from the setup code, on a second computer joined with an account code and a two-factor
  /// identity, as making invites needs; returns that device token.
  pub async fn admin(&self) -> SecretValue {
    let code = self
      .server
      .host_setup_code()
      .await
      .expect("setup code")
      .expect("no admin yet");
    self.join_with(&code, Some("alice"), (None, None)).await;
    let account = self
      .server
      .host_account_code("alice".into())
      .await
      .expect("account code");
    token(&self.join_as(&account, "alice").await.token)
  }

  /// `username`, invited to the server by `admin` with a code and joined: their id and token.
  pub async fn person(&self, admin: &SecretValue, username: &str) -> (UserId, SecretValue) {
    let invite = self
      .post(admin, "/invites", json!({"kind":"server","delivery":"code"}))
      .await;
    let code = Credential::new(invite["code"].as_str().expect("invite code").into());
    let joined = self.join(&code, username).await;
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
