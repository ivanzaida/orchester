pub mod args;
pub mod http;
pub mod log;
pub mod mailer;
pub mod postgres;
pub mod relay;
pub mod team;

use std::{
  net::SocketAddr,
  sync::{
    Arc, OnceLock,
    atomic::{AtomicI64, Ordering},
  },
  time::Duration,
};

use orchester_api::{
  Credential,
  cloud::CloudSignedIn,
  identity::{PrivateKey, RootKey},
};
use orchester_cloud::{Cloud, addresses::TrustedProxy, clock::Clock, config::Config, error::CloudError, mail::Mailer};
use parking_lot::Mutex;
use serde_json::{Value, json};
use tokio::{sync::oneshot, task::JoinHandle};

pub use self::{http::Response, mailer::RecordingMailer, postgres::Postgres, team::TeamServer};

pub const PASSWORD: &str = "correct horse battery staple";
pub const BASE_URL: &str = "https://cloud.example.test";

static POSTGRES: OnceLock<Postgres> = OnceLock::new();
static SECRETS: Mutex<Vec<String>> = Mutex::new(Vec::new());

pub fn set_postgres(postgres: Postgres) -> &'static Postgres {
  if let Some(password) = &postgres.password {
    remember(password);
  }
  POSTGRES.get_or_init(|| postgres)
}

pub fn postgres() -> &'static Postgres {
  POSTGRES.get().expect("the run starts PostgreSQL before any test")
}

/// Every secret a test saw; the run's log must contain none of them.
pub fn remember(secret: &str) {
  SECRETS.lock().push(secret.to_owned());
}

pub fn secrets() -> Vec<String> {
  SECRETS.lock().clone()
}

pub struct TestClock(AtomicI64);

impl TestClock {
  pub fn at(millis: i64) -> Self {
    Self(AtomicI64::new(millis))
  }

  pub fn advance(&self, millis: i64) {
    self.0.fetch_add(millis, Ordering::SeqCst);
  }
}

impl Clock for TestClock {
  fn now_ms(&self) -> i64 {
    self.0.load(Ordering::SeqCst)
  }
}

pub const MINUTE: i64 = 60 * 1000;
pub const HOUR: i64 = 60 * MINUTE;
pub const DAY: i64 = 24 * HOUR;

pub struct TestCloud {
  pub cloud: Cloud,
  pub clock: Arc<TestClock>,
  pub mail: Arc<RecordingMailer>,
  pub root: RootKey,
  pub database: String,
  address: SocketAddr,
  stop: Option<oneshot::Sender<()>>,
  task: Option<JoinHandle<Result<(), CloudError>>>,
}

pub fn config(database: &str, root: &RootKey, trusted_proxies: Vec<TrustedProxy>) -> Config {
  let key_encryption_key = uuid::Uuid::new_v4().simple().to_string() + &uuid::Uuid::new_v4().simple().to_string();
  remember(&key_encryption_key);
  Config {
    listen: "127.0.0.1:0".parse().expect("loopback"),
    public_url: BASE_URL.into(),
    database_url: Credential::new(postgres().url(database)),
    // The tests' cloud sends through `RecordingMailer`; the URL is only validated.
    smtp_url: Credential::new("smtp://127.0.0.1:25".into()),
    smtp_from: "Orchester <no-reply@cloud.example.test>".into(),
    key_encryption_key: Credential::new(key_encryption_key),
    root_public_key: root.public_key(),
    trusted_proxies,
  }
}

impl TestCloud {
  pub async fn start() -> Self {
    Self::start_with(Vec::new()).await
  }

  /// A cloud on loopback with its own database, clock and mailer; `trusted_proxies` lets a test speak for several
  /// client addresses through `X-Forwarded-For`.
  pub async fn start_with(trusted_proxies: Vec<TrustedProxy>) -> Self {
    let mail = Arc::new(RecordingMailer::default());
    Self::open(trusted_proxies, mail.clone(), mail).await
  }

  /// A cloud that sends through `mailer`; `mail` then stays empty.
  pub async fn start_with_mailer(mailer: Arc<dyn Mailer>) -> Self {
    Self::open(Vec::new(), mailer, Arc::new(RecordingMailer::default())).await
  }

  async fn open(trusted_proxies: Vec<TrustedProxy>, mailer: Arc<dyn Mailer>, mail: Arc<RecordingMailer>) -> Self {
    let database = postgres().create_database().await;
    let private = PrivateKey::generate().expect("root key");
    remember(
      &private
        .pkcs8()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>(),
    );
    let root = RootKey::new(&private).expect("root");
    let clock = Arc::new(TestClock(AtomicI64::new(1_791_000_000_000)));
    let cloud = Cloud::open(config(&database, &root, trusted_proxies), clock.clone(), mailer)
      .await
      .expect("open the cloud");
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("listen on loopback");
    let address = listener.local_addr().expect("address");
    let (stop, stopped) = oneshot::channel::<()>();
    let task = tokio::spawn(orchester_cloud::serve(cloud.clone(), listener, async {
      // An explicit stop and a dropped fixture both end the server.
      stopped.await.unwrap_or(());
    }));
    Self {
      cloud,
      clock,
      mail,
      root,
      database,
      address,
      stop: Some(stop),
      task: Some(task),
    }
  }

  pub fn address(&self) -> SocketAddr {
    self.address
  }

  pub async fn finish(mut self) {
    self.stop.take().expect("running").send(()).expect("the server listens");
    tokio::time::timeout(Duration::from_secs(30), self.task.take().expect("server task"))
      .await
      .expect("shutdown deadline")
      .expect("join the server")
      .expect("serve result");
    postgres().drop_database(&self.database).await;
  }

  pub async fn request(&self, method: &str, path: &str, token: Option<&str>, body: Option<Value>) -> Response {
    self.request_headers(method, path, token, body, &[]).await
  }

  pub async fn request_headers(
    &self,
    method: &str,
    path: &str,
    token: Option<&str>,
    body: Option<Value>,
    extra: &[(&str, &str)],
  ) -> Response {
    let body = body
      .map(|body| serde_json::to_vec(&body).expect("JSON"))
      .unwrap_or_default();
    let bearer = token.map(|token| format!("Bearer {token}"));
    let mut headers = vec![("Content-Type", "application/json")];
    if let Some(bearer) = &bearer {
      headers.push(("Authorization", bearer));
    }
    headers.extend_from_slice(extra);
    http::exchange(self.address, method, &format!("/api/v1{path}"), &headers, &body).await
  }

  pub async fn raw(&self, method: &str, path: &str, headers: &[(&str, &str)], body: &[u8]) -> Response {
    http::exchange(self.address, method, path, headers, body).await
  }

  pub async fn sign_up(&self, email: &str, password: &str) -> Response {
    remember(password);
    remember(email.trim());
    remember(&email.trim().to_lowercase());
    self
      .request(
        "POST",
        "/auth/signup",
        None,
        Some(json!({"email": email, "password": password, "display_name": "Ada Lovelace"})),
      )
      .await
  }

  /// Signs up and confirms through the link this sign-up emailed, as the web front's confirmation page does.
  pub async fn register(&self, email: &str) {
    let before = self.mail.count(email);
    assert_eq!(self.sign_up(email, PASSWORD).await.status, 202);
    let token = self.mail.nth(email, before + 1).await.link_token();
    assert_eq!(self.confirm(&token, PASSWORD).await.status, 204);
  }

  /// The web front's confirmation page: the link's token and the password of the sign-up it was sent for.
  pub async fn confirm(&self, token: &str, password: &str) -> Response {
    remember(password);
    let body = json!({"link_token": token, "password": password});
    self.request("POST", "/auth/confirm", None, Some(body)).await
  }

  pub async fn try_sign_in(&self, email: &str, password: &str, device: &str) -> Response {
    remember(password);
    remember(email);
    self
      .request(
        "POST",
        "/auth/sessions",
        None,
        Some(json!({"email": email, "password": password, "device": {"name": device, "platform": "windows"}})),
      )
      .await
  }

  pub async fn sign_in(&self, email: &str, password: &str, device: &str) -> CloudSignedIn {
    let response = self.try_sign_in(email, password, device).await;
    assert_eq!(response.status, 201, "sign-in answered {}", response.status);
    let signed: CloudSignedIn = response.json();
    remember(signed.token.expose());
    signed
  }

  /// Links `team` for the account of the device `token`, as its owner carries the link statement.
  pub async fn link_server(&self, token: &str, team: &TeamServer) -> Response {
    use orchester_cloud::clock::Clock;
    let link = team.link("Kontur team", self.clock.now_ms());
    self
      .request("POST", "/servers", Some(token), Some(json!({"link": link})))
      .await
  }

  /// The whole key ceremony: a new signing key, its statement signed by the root, published. Returns what was
  /// published.
  pub async fn publish_keys(&self) -> String {
    self.cloud.host_rotate_keys().await.expect("rotate");
    self.publish_key_set().await
  }

  /// The weekly ceremony without a rotation.
  pub async fn publish_key_set(&self) -> String {
    let statement = self.cloud.host_prepare_key_set().await.expect("prepare");
    let signed = self.root.sign_key_set(&statement, statement.issued_at).expect("sign");
    self.cloud.host_publish_key_set(&signed).await.expect("publish");
    signed
  }
}

impl Drop for TestCloud {
  fn drop(&mut self) {
    if let Some(task) = self.task.take() {
      task.abort();
    }
  }
}
