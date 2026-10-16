//! Orchester Cloud from this tree (`orchester-cloud`) on loopback, on a database of its own in the PostgreSQL that
//! `ORCHESTER_CLOUD_TEST_DATABASE_URL` names, with a mailer that keeps what it sends and the key ceremony done: what
//! the desktop's N1 flows talk to in `tests/orchester_cloud`. The database is dropped at the end.

use std::{net::SocketAddr, sync::Arc, time::Duration};

use orchester_api::{
  Credential,
  client::{CloudClient, ServerAddress},
  cloud::{CloudPlatform, CloudSignIn, DeviceInfo, ServerLinked},
  identity::{PrivateKey, RootKey},
};
use orchester_cloud::{
  Cloud,
  clock::SystemClock,
  config::Config,
  mail::{Email, MailFuture, Mailer},
};
use orchester_core::id::IdempotencyKey;
use parking_lot::Mutex;
use serde_json::{Value, json};
use tokio::{
  io::{AsyncReadExt, AsyncWriteExt},
  net::TcpStream,
  runtime::Runtime,
  sync::oneshot,
};

/// The variable that names the PostgreSQL server (a URL without a database, whose user may create databases).
pub const DATABASE_URL: &str = "ORCHESTER_CLOUD_TEST_DATABASE_URL";

pub struct RealCloud {
  runtime: Runtime,
  address: SocketAddr,
  inbox: Arc<Inbox>,
  server: String,
  database: String,
  stop: Option<oneshot::Sender<()>>,
}

impl RealCloud {
  /// The cloud on a new database of the PostgreSQL `ORCHESTER_CLOUD_TEST_DATABASE_URL` names, keys published.
  pub fn start() -> Self {
    let server = std::env::var(DATABASE_URL)
      .unwrap_or_else(|_| panic!("{DATABASE_URL} names the PostgreSQL these tests run against"))
      .trim_end_matches('/')
      .to_owned();
    let runtime = Runtime::new().expect("cloud runtime");
    let database = format!("n1_{}", IdempotencyKey::generate().to_string().replace('-', ""));
    let inbox = Arc::new(Inbox::default());
    let root = RootKey::new(&PrivateKey::generate().expect("root key")).expect("root");
    let (address, stop) = runtime.block_on(async {
      admin(&server)
        .await
        .batch_execute(&format!("CREATE DATABASE {database}"))
        .await
        .expect("database");
      let config = Config {
        listen: "127.0.0.1:0".parse().expect("loopback"),
        public_url: "https://cloud.example.test".into(),
        database_url: Credential::new(format!("{server}/{database}?sslmode=disable")),
        smtp_url: Credential::new("smtp://127.0.0.1:25".into()),
        smtp_from: "Orchester <no-reply@cloud.example.test>".into(),
        key_encryption_key: Credential::new("ab".repeat(32)),
        root_public_key: root.public_key(),
        trusted_proxies: Vec::new(),
      };
      let cloud = Cloud::open(config, Arc::new(SystemClock), inbox.clone())
        .await
        .expect("open the cloud");
      cloud.host_rotate_keys().await.expect("a signing key");
      let statement = cloud.host_prepare_key_set().await.expect("key set");
      let signed = root.sign_key_set(&statement, statement.issued_at).expect("signed");
      cloud.host_publish_key_set(&signed).await.expect("published");
      let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("listen");
      let address = listener.local_addr().expect("address");
      let (stop, stopped) = oneshot::channel::<()>();
      tokio::spawn(orchester_cloud::serve(cloud, listener, async {
        stopped.await.unwrap_or(());
      }));
      (address, stop)
    });
    Self {
      runtime,
      address,
      inbox,
      server,
      database,
      stop: Some(stop),
    }
  }

  pub fn url(&self) -> String {
    format!("http://localhost:{}", self.address.port())
  }

  /// A confirmed account: signed up, its confirmation link used with its password.
  pub fn account(&self, email: &str, password: &str, name: &str) {
    let body = json!({"email": email, "password": password, "display_name": name});
    assert_eq!(self.request("POST", "/auth/signup", None, &body).0, 202);
    let link = self.runtime.block_on(self.inbox.link_to(email));
    let confirm = json!({"link_token": link, "password": password});
    assert_eq!(self.request("POST", "/auth/confirm", None, &confirm).0, 204);
  }

  /// A device of the account, signed in as a desktop would.
  pub fn device(&self, email: &str, password: &str) -> Credential {
    let client = CloudClient::new(ServerAddress::parse(&self.url()).expect("address")).expect("client");
    let request = CloudSignIn {
      email: email.into(),
      password: Credential::new(password.into()),
      device: DeviceInfo {
        name: "test computer".into(),
        platform: CloudPlatform::Windows,
      },
    };
    self
      .runtime
      .block_on(client.sign_in(&request))
      .expect("signed in")
      .token
  }

  /// Registers the team server that signed `statement`, for the account of `device`: what the server pins.
  pub fn link(&self, device: &Credential, statement: &str) -> ServerLinked {
    let (status, answer) = self.request("POST", "/servers", Some(device.expose()), &json!({"link": statement}));
    assert_eq!(status, 201, "{answer}");
    serde_json::from_value(answer).expect("linked")
  }

  /// Delivers an invite the team server signed, as its admin does.
  pub fn deliver(&self, signed: &str) {
    assert_eq!(
      self.request("POST", "/invites", None, &json!({"invite": signed})).0,
      202
    );
  }

  fn request(&self, method: &str, path: &str, token: Option<&str>, body: &Value) -> (u16, Value) {
    self.runtime.block_on(exchange(self.address, method, path, token, body))
  }
}

impl Drop for RealCloud {
  fn drop(&mut self) {
    if let Some(stop) = self.stop.take() {
      // The cloud may have stopped already; its database goes either way.
      let _ = stop.send(());
    }
    let (server, database) = (self.server.clone(), self.database.clone());
    self.runtime.block_on(async move {
      // Let the cloud's pool close its connections before the database goes.
      tokio::time::sleep(Duration::from_millis(200)).await;
      let drop = format!("DROP DATABASE IF EXISTS {database} WITH (FORCE)");
      if let Err(error) = admin(&server).await.batch_execute(&drop).await {
        eprintln!("the test database {database} stays: {error}");
      }
    });
  }
}

/// A connection to the server's `postgres` database.
async fn admin(server: &str) -> tokio_postgres::Client {
  let (client, connection) =
    tokio_postgres::connect(&format!("{server}/postgres?sslmode=disable"), tokio_postgres::NoTls)
      .await
      .expect("connect to PostgreSQL");
  tokio::spawn(async move {
    if connection.await.is_err() {
      eprintln!("a connection to the test PostgreSQL ended with an error");
    }
  });
  client
}

/// The cloud's mail, kept for the test to read the links in it.
#[derive(Default)]
struct Inbox(Mutex<Vec<(String, String)>>);

impl Mailer for Inbox {
  fn send(&self, email: Email) -> MailFuture<'_> {
    self.0.lock().push((email.to().to_owned(), email.body().to_owned()));
    Box::pin(async { Ok(()) })
  }
}

impl Inbox {
  /// The token of the newest confirmation link sent to `email`, waiting for it to be sent in the background.
  async fn link_to(&self, email: &str) -> String {
    for _ in 0..100 {
      let found = self
        .0
        .lock()
        .iter()
        .rev()
        .find(|(to, _)| to == email)
        .and_then(|(_, body)| {
          let start = body.find("/confirm/")? + "/confirm/".len();
          body.get(start..start + 64).map(str::to_owned)
        });
      if let Some(token) = found {
        return token;
      }
      tokio::time::sleep(Duration::from_millis(100)).await;
    }
    panic!("no confirmation reached {email}");
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
