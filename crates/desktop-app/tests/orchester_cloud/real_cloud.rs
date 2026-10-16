//! Orchester Cloud from this tree on loopback: the `orchester-cloud` program (`cargo build -p orchester-cloud`), run
//! as its own process so the app's tests do not link the cloud's web stack, on a database of its own in the PostgreSQL
//! that `ORCHESTER_CLOUD_TEST_DATABASE_URL` names, sending its mail to an SMTP stub, with the key ceremony done as an
//! operator does it. The process is stopped and the database dropped at the end.

use std::{
  net::{SocketAddr, TcpListener},
  path::PathBuf,
  process::{Child, Command, Stdio},
  sync::Arc,
  time::Duration,
};

use orchester_api::{
  Credential,
  client::{CloudClient, ServerAddress},
  cloud::{CloudPlatform, CloudSignIn, DeviceInfo, ServerLinked},
  identity::{KeySetStatement, PrivateKey, RootKey},
};
use orchester_core::id::IdempotencyKey;
use parking_lot::Mutex;
use serde_json::{Value, json};
use tempfile::TempDir;
use tokio::{
  io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
  net::TcpStream,
  runtime::Runtime,
};

/// The variable that names the PostgreSQL server (a URL without a database, whose user may create databases).
pub const DATABASE_URL: &str = "ORCHESTER_CLOUD_TEST_DATABASE_URL";

pub struct RealCloud {
  runtime: Runtime,
  address: SocketAddr,
  mail: Arc<Mutex<Vec<String>>>,
  server: String,
  database: String,
  process: Child,
  _scratch: TempDir,
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
    runtime.block_on(async {
      admin(&server)
        .await
        .batch_execute(&format!("CREATE DATABASE {database}"))
        .await
        .expect("database");
    });
    let mail = Arc::new(Mutex::new(Vec::new()));
    let relay = runtime.block_on(relay(mail.clone()));
    let scratch = tempfile::Builder::new()
      .prefix("cloud-")
      .tempdir_in(scratch_root())
      .expect("scratch");
    let root = RootKey::new(&PrivateKey::generate().expect("root key")).expect("root");
    let address = free_address();
    let environment = environment((&server, &database), (address, relay), &root.public_key().encoded());
    ceremony(&scratch, &environment, &root);
    let process = Command::new(program())
      .arg("serve")
      .current_dir(scratch.path())
      .envs(environment)
      .stdout(Stdio::null())
      // Its request log is the cloud's own business; a failing test says what it saw.
      .stderr(Stdio::null())
      .spawn()
      .expect("start orchester-cloud");
    let cloud = Self {
      runtime,
      address,
      mail,
      server,
      database,
      process,
      _scratch: scratch,
    };
    cloud.wait_until_listening();
    cloud
  }

  pub fn url(&self) -> String {
    format!("http://localhost:{}", self.address.port())
  }

  /// A confirmed account: signed up, its confirmation link used with its password.
  pub fn account(&self, email: &str, password: &str, name: &str) {
    let body = json!({"email": email, "password": password, "display_name": name});
    assert_eq!(self.request("POST", "/auth/signup", None, &body).0, 202);
    let link = self.confirmation_to(email);
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

  fn wait_until_listening(&self) {
    for _ in 0..300 {
      if std::net::TcpStream::connect(self.address).is_ok() {
        return;
      }
      std::thread::sleep(Duration::from_millis(100));
    }
    panic!("orchester-cloud did not listen at {}", self.address);
  }

  /// The token of the newest confirmation link the relay received for `email`.
  fn confirmation_to(&self, email: &str) -> String {
    for _ in 0..100 {
      let found = self.mail.lock().iter().rev().find_map(|message| {
        // Quoted-printable wraps long lines with a soft break.
        let text = message.replace("=\r\n", "").replace("=\n", "");
        if !text.contains(email) {
          return None;
        }
        let start = text.find("/confirm/")? + "/confirm/".len();
        text.get(start..start + 64).map(str::to_owned)
      });
      if let Some(token) = found {
        return token;
      }
      std::thread::sleep(Duration::from_millis(100));
    }
    panic!("no confirmation reached {email}");
  }
}

impl Drop for RealCloud {
  fn drop(&mut self) {
    // Our own child, stopped by its handle; it may have stopped already.
    if let Err(error) = self.process.kill() {
      eprintln!("orchester-cloud had stopped: {error}");
    }
    if let Err(error) = self.process.wait() {
      eprintln!("orchester-cloud could not be waited for: {error}");
    }
    let (server, database) = (self.server.clone(), self.database.clone());
    self.runtime.block_on(async move {
      let drop = format!("DROP DATABASE IF EXISTS {database} WITH (FORCE)");
      if let Err(error) = admin(&server).await.batch_execute(&drop).await {
        eprintln!("the test database {database} stays: {error}");
      }
    });
  }
}

/// The weekly ceremony on a new cloud: a signing key, its key set signed offline by `root`, published.
fn ceremony(scratch: &TempDir, environment: &[(String, String)], root: &RootKey) {
  let run = |args: &[&str]| {
    let status = Command::new(program())
      .args(args)
      .current_dir(scratch.path())
      .envs(environment.iter().cloned())
      .stdout(Stdio::null())
      .status()
      .expect("run orchester-cloud");
    assert!(status.success(), "orchester-cloud {args:?} failed");
  };
  run(&["keys", "rotate"]);
  let statement = scratch.path().join("statement.json");
  run(&["keys", "prepare", path(&statement)]);
  let prepared: KeySetStatement =
    serde_json::from_slice(&std::fs::read(&statement).expect("statement")).expect("a key set statement");
  let signed = scratch.path().join("signed.jws");
  let compact = root.sign_key_set(&prepared, prepared.issued_at).expect("signed");
  std::fs::write(&signed, compact).expect("write the signed key set");
  run(&["keys", "publish", path(&signed)]);
}

/// The program built from this tree, beside the test binaries' folder (`target/<profile>/`).
fn program() -> PathBuf {
  let deps = std::env::current_exe().expect("the test binary");
  let profile = deps.parent().and_then(|deps| deps.parent()).expect("target/<profile>");
  let program = profile.join(format!("orchester-cloud{}", std::env::consts::EXE_SUFFIX));
  assert!(
    program.is_file(),
    "build the cloud first: cargo build -p orchester-cloud ({} is missing)",
    program.display()
  );
  program
}

fn scratch_root() -> PathBuf {
  let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.tmp/desktop-sharing/tests");
  std::fs::create_dir_all(&root).expect("scratch directory");
  root
}

fn path(path: &std::path::Path) -> &str {
  path.to_str().expect("a UTF-8 path")
}

fn free_address() -> SocketAddr {
  TcpListener::bind("127.0.0.1:0")
    .and_then(|listener| listener.local_addr())
    .expect("a free port")
}

/// The cloud's configuration, as `ORCHESTER_CLOUD_*` variables.
fn environment(
  (server, database): (&str, &str),
  (listen, relay): (SocketAddr, SocketAddr),
  root_public_key: &str,
) -> Vec<(String, String)> {
  [
    ("LISTEN", listen.to_string()),
    ("PUBLIC_URL", "https://cloud.example.test".to_owned()),
    ("DATABASE_URL", format!("{server}/{database}?sslmode=disable")),
    ("SMTP_URL", format!("smtp://{relay}")),
    ("SMTP_FROM", "Orchester <no-reply@cloud.example.test>".to_owned()),
    ("KEY_ENCRYPTION_KEY", "ab".repeat(32)),
    ("ROOT_PUBLIC_KEY", root_public_key.to_owned()),
  ]
  .into_iter()
  .map(|(key, value)| (format!("ORCHESTER_CLOUD_{key}"), value))
  .collect()
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

/// An SMTP relay on loopback that accepts every message and keeps its data.
async fn relay(mail: Arc<Mutex<Vec<String>>>) -> SocketAddr {
  let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
    .await
    .expect("relay listens");
  let address = listener.local_addr().expect("relay address");
  tokio::spawn(async move {
    while let Ok((stream, _)) = listener.accept().await {
      tokio::spawn(session(stream, mail.clone()));
    }
  });
  address
}

async fn session(stream: TcpStream, mail: Arc<Mutex<Vec<String>>>) {
  let (read, mut write) = stream.into_split();
  let mut lines = BufReader::new(read).lines();
  if write.write_all(b"220 relay.test ESMTP\r\n").await.is_err() {
    return;
  }
  while let Ok(Some(line)) = lines.next_line().await {
    let command = line.to_ascii_uppercase();
    let reply: &[u8] = if command.starts_with("EHLO") || command.starts_with("HELO") {
      b"250-relay.test\r\n250 8BITMIME\r\n"
    } else if command.starts_with("DATA") {
      if write.write_all(b"354 go ahead\r\n").await.is_err() {
        return;
      }
      let mut data = String::new();
      while let Ok(Some(line)) = lines.next_line().await {
        if line == "." {
          break;
        }
        data.push_str(&line);
        data.push('\n');
      }
      mail.lock().push(data);
      b"250 queued\r\n"
    } else if command.starts_with("QUIT") {
      // The client goes either way.
      let _ = write.write_all(b"221 bye\r\n").await;
      return;
    } else {
      b"250 ok\r\n"
    };
    if write.write_all(reply).await.is_err() {
      return;
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
