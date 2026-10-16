//! A stand-in for Orchester Cloud on loopback: it answers `GET /api/v1/keys` with a key set its root signed and
//! `POST /api/v1/identity-tokens` with a fresh identity token for one person and one invite, signed with
//! `orchester-api`'s identity keys. Nothing else; enough for a desktop's cloud join.

use std::{
  io::{Read, Write},
  net::{SocketAddr, TcpListener},
  sync::{
    Arc, OnceLock,
    atomic::{AtomicUsize, Ordering},
  },
  thread,
  time::{SystemTime, UNIX_EPOCH},
};

use orchester_api::{
  cloud::{CloudPlatform, DeviceInfo},
  identity::{IdentityClaims, KeySetStatement, ListedKey, PrivateKey, RootKey, SigningKey},
};
use orchester_core::id::IdempotencyKey;
use serde_json::json;

const DAY: i64 = 24 * 60 * 60;

/// Who the cloud says the person is, and the invite their token accepts.
pub struct Person {
  pub account: String,
  pub email: String,
  pub name: String,
  pub invite: Option<String>,
}

pub struct FakeCloud {
  pub address: SocketAddr,
  /// The root's public key, which the team server pins when it is linked.
  pub root_key: String,
  pub key_set: String,
  /// The team server the tokens are for: known once the server is linked to this cloud.
  audience: Arc<OnceLock<String>>,
  /// The identity tokens issued so far.
  issued: Arc<AtomicUsize>,
}

impl FakeCloud {
  /// A cloud that issues tokens for `person`, to the team server named by [`Self::serve_for`].
  pub fn start(person: Person) -> Self {
    let (root, signing) = (
      PrivateKey::generate().expect("root"),
      PrivateKey::generate().expect("key"),
    );
    let root = RootKey::new(&root).expect("root key");
    let signing = SigningKey::new(&signing).expect("signing key");
    let now = now();
    let listed = ListedKey {
      kid: signing.kid().to_owned(),
      public_key: signing.public_key().encoded(),
      not_before: now - DAY,
      signs_until: now + 29 * DAY,
      listed_until: now + 89 * DAY,
    };
    let statement = KeySetStatement::new("https://cloud.test".into(), vec![listed], now);
    let key_set = root.sign_key_set(&statement, now).expect("key set");
    let listener = TcpListener::bind("127.0.0.1:0").expect("listen");
    let address = listener.local_addr().expect("address");
    let served = key_set.clone();
    let audience = Arc::new(OnceLock::<String>::new());
    let serving = audience.clone();
    let issued = Arc::new(AtomicUsize::new(0));
    let counting = issued.clone();
    thread::spawn(move || {
      for stream in listener.incoming() {
        let Ok(mut stream) = stream else { return };
        let mut request = [0_u8; 8192];
        let read = stream.read(&mut request).unwrap_or(0);
        let line = String::from_utf8_lossy(&request[..read])
          .lines()
          .next()
          .unwrap_or_default()
          .to_owned();
        let (status, body) = if line.starts_with("GET /api/v1/keys") {
          ("200 OK", json!({"key_set": served}).to_string())
        } else if line.starts_with("POST /api/v1/identity-tokens") {
          let audience = serving.get().expect("the server is linked");
          counting.fetch_add(1, Ordering::SeqCst);
          ("201 Created", token(&signing, audience, &person))
        } else {
          (
            "404 Not Found",
            json!({"error":"not_found","message":"Not found"}).to_string(),
          )
        };
        let answer = format!(
          "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
          body.len()
        );
        // The test reads the answer or fails on its own; a broken connection needs nothing here.
        let _ = stream.write_all(answer.as_bytes());
      }
    });
    Self {
      address,
      root_key: root.public_key().encoded(),
      key_set,
      audience,
      issued,
    }
  }

  /// How many identity tokens the cloud issued.
  pub fn issued(&self) -> usize {
    self.issued.load(Ordering::SeqCst)
  }

  /// Issues tokens for the team server `audience` from now on.
  pub fn serve_for(&self, audience: String) {
    self.audience.set(audience).expect("one server");
  }

  pub fn url(&self) -> String {
    format!("http://localhost:{}", self.address.port())
  }
}

fn token(signing: &SigningKey, audience: &str, person: &Person) -> String {
  let iat = now();
  let claims = IdentityClaims {
    iss: "https://cloud.test".into(),
    aud: audience.into(),
    sub: person.account.parse().expect("a cloud account id"),
    email: person.email.clone(),
    name: person.name.clone(),
    device: DeviceInfo {
      name: "Bohdan's laptop".into(),
      platform: CloudPlatform::Windows,
    },
    invite: person.invite.clone(),
    purpose: None,
    // The person's device signed in with two-factor authentication, which accepting an invite needs.
    mfa: true,
    jti: IdempotencyKey::generate().to_string(),
    iat,
    exp: iat + 300,
  };
  let token = signing.sign_token(&claims).expect("token");
  json!({"identity_token": token.expose(), "expires_at": claims.exp}).to_string()
}

fn now() -> i64 {
  let elapsed = SystemTime::now().duration_since(UNIX_EPOCH).expect("after 1970");
  i64::try_from(elapsed.as_secs()).expect("seconds fit")
}
