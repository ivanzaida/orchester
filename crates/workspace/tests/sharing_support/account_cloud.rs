//! A stand-in for Orchester Cloud's account API on loopback, as `docs/architecture/server.md` names it: sign-in and
//! sign-out of a device, the account's invites (list, accept with a one-time identity token, decline) and team
//! servers, the key set and identity tokens, all signed with `orchester-api`'s identity keys. With `offers` off it
//! answers the invite and server endpoints `404`, as today's cloud does.

#![allow(
  dead_code,
  reason = "shared by test binaries and a QA example, each of which uses a part"
)]

use std::{
  io::{Read, Write},
  net::{SocketAddr, TcpListener, TcpStream},
  sync::Arc,
  thread,
  time::{SystemTime, UNIX_EPOCH},
};

use orchester_api::identity::{IdentityClaims, KeySetStatement, ListedKey, PrivateKey, RootKey, SigningKey};
use orchester_core::id::IdempotencyKey;
use parking_lot::Mutex;
use serde_json::{Value, json};

const DAY: i64 = 24 * 60 * 60;
pub const ACCOUNT: &str = "0191c7c0-0000-7000-8000-000000000b0b";
pub const EMAIL: &str = "bohdan@example.com";
pub const PASSWORD: &str = "correct horse battery";
const TOKEN: &str = "orcc_0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

/// What the cloud holds: the team server it names, the invites waiting, and what was asked of it.
#[derive(Default)]
struct State {
  /// `{ id, name, address, fingerprint }` of the team server.
  server: Option<Value>,
  /// Open invites, as `GET /invites` lists them.
  invites: Vec<Value>,
  declined: Vec<String>,
  signed_out: bool,
}

pub struct AccountCloud {
  pub address: SocketAddr,
  pub root_key: String,
  pub key_set: String,
  state: Arc<Mutex<State>>,
}

impl AccountCloud {
  /// A cloud whose invite and server endpoints answer when `offers` holds.
  pub fn start(offers: bool) -> Self {
    let root = RootKey::new(&PrivateKey::generate().expect("root")).expect("root key");
    let signing = Arc::new(SigningKey::new(&PrivateKey::generate().expect("key")).expect("signing key"));
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
    let state = Arc::new(Mutex::new(State::default()));
    let serving = (state.clone(), key_set.clone());
    thread::spawn(move || {
      for stream in listener.incoming().flatten() {
        serve(stream, &serving.0, &serving.1, &signing, offers);
      }
    });
    Self {
      address,
      root_key: root.public_key().encoded(),
      key_set,
      state,
    }
  }

  pub fn url(&self) -> String {
    format!("http://localhost:{}", self.address.port())
  }

  /// Names the team server `audience` at `address`, trusting `fingerprint` (hex SHA-256) when it has its own
  /// certificate.
  pub fn name_server(&self, audience: &str, address: &str, fingerprint: Option<&str>) {
    self.state.lock().server = Some(json!({
      "id": audience, "name": "Kontur team", "address": address, "fingerprint": fingerprint,
    }));
  }

  /// An invite from Alice to `project` (its name and address) on the named server, or to the server alone, carrying
  /// the team server's invite id `invite`.
  pub fn invite(&self, invite: &str, project: Option<(&str, &str)>) {
    let mut state = self.state.lock();
    let server = state.server.clone().expect("the server is named first");
    let now = now() * 1000;
    let project = project.map(|(name, address)| json!({"address": address, "name": name}));
    state.invites.push(json!({
      "id": invite, "server": server, "kind": if project.is_some() { "project" } else { "server" },
      "project": project, "role": project.as_ref().map(|_| "member"), "inviter": "Alice Moreau",
      "created_at": now, "expires_at": now + 7 * DAY * 1000,
    }));
  }

  pub fn declined(&self) -> Vec<String> {
    self.state.lock().declined.clone()
  }

  pub fn signed_out(&self) -> bool {
    self.state.lock().signed_out
  }
}

fn serve(mut stream: TcpStream, state: &Mutex<State>, key_set: &str, signing: &SigningKey, offers: bool) {
  let Some((line, headers, body)) = read_request(&mut stream) else {
    return;
  };
  let authorized = headers.contains(&format!("authorization: bearer {TOKEN}"));
  let path = line.split_whitespace().take(2).collect::<Vec<_>>().join(" ");
  let (status, answer) = route(&path, (authorized, offers, &body), state, (key_set, signing));
  let answer = answer.map(|value| value.to_string()).unwrap_or_default();
  let reply = format!(
    "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{answer}",
    answer.len()
  );
  // The test reads the answer or fails on its own; a broken connection needs nothing here.
  let _ = stream.write_all(reply.as_bytes());
}

type Answer = (&'static str, Option<Value>);

fn route(
  path: &str,
  (authorized, offers, body): (bool, bool, &str),
  state: &Mutex<State>,
  (key_set, signing): (&str, &SigningKey),
) -> Answer {
  let unauthorized = ("401 Unauthorized", Some(error("unauthorized")));
  let not_found = ("404 Not Found", Some(error("not_found")));
  match path {
    "GET /api/v1/keys" => ("200 OK", Some(json!({"key_set": key_set}))),
    "POST /api/v1/auth/sessions" => sign_in(body),
    _ if !authorized => unauthorized,
    "POST /api/v1/identity-tokens" => {
      let request: Value = serde_json::from_str(body).unwrap_or(Value::Null);
      let audience = request["audience"].as_str().unwrap_or_default();
      let token = identity_token(signing, audience, None);
      (
        "201 Created",
        Some(json!({"identity_token": token, "expires_at": now() + 300})),
      )
    }
    "DELETE /api/v1/auth/sessions/current" => {
      state.lock().signed_out = true;
      ("204 No Content", None)
    }
    _ if !offers => not_found,
    "GET /api/v1/invites" => ("200 OK", Some(json!({"invites": state.lock().invites}))),
    "GET /api/v1/servers" => {
      let servers: Vec<Value> = state.lock().server.iter().cloned().collect();
      ("200 OK", Some(json!({"servers": servers})))
    }
    _ => invite_action(path, state, signing).unwrap_or(not_found),
  }
}

fn sign_in(body: &str) -> Answer {
  let request: Value = serde_json::from_str(body).unwrap_or(Value::Null);
  if request["email"] != EMAIL || request["password"] != PASSWORD {
    return ("401 Unauthorized", Some(error("invalid_credentials")));
  }
  let account = json!({"id": ACCOUNT, "email": EMAIL, "display_name": "Bohdan Koval", "created_at": 1});
  let device = json!({"id": "0191c7c0-0000-7000-8000-00000000d0d0", "name": request["device"]["name"],
    "platform": request["device"]["platform"], "created_at": 1, "last_used_at": 1, "current": true});
  (
    "201 Created",
    Some(json!({"token": TOKEN, "device": device, "account": account})),
  )
}

/// `POST /api/v1/invites/{id}/accept` and `…/decline`; `None` for any other path or an invite not open.
fn invite_action(path: &str, state: &Mutex<State>, signing: &SigningKey) -> Option<Answer> {
  let rest = path.strip_prefix("POST /api/v1/invites/")?;
  let (id, action) = rest.split_once('/')?;
  let mut state = state.lock();
  let position = state.invites.iter().position(|invite| invite["id"] == id)?;
  let invite = state.invites.remove(position);
  match action {
    "decline" => {
      state.declined.push(id.to_owned());
      Some(("204 No Content", None))
    }
    "accept" => {
      let server = invite["server"].clone();
      let audience = server["id"].as_str()?.to_owned();
      let token = identity_token(signing, &audience, Some(id));
      let answer = json!({"server": server, "project": invite["project"], "identity_token": token});
      Some(("200 OK", Some(answer)))
    }
    _ => None,
  }
}

fn identity_token(signing: &SigningKey, audience: &str, invite: Option<&str>) -> String {
  let iat = now();
  let claims = IdentityClaims {
    iss: "https://cloud.test".into(),
    aud: audience.into(),
    sub: ACCOUNT.parse().expect("a cloud account id"),
    email: EMAIL.into(),
    name: "Bohdan Koval".into(),
    device: orchester_api::cloud::DeviceInfo {
      name: "Bohdan's laptop".into(),
      platform: orchester_api::cloud::CloudPlatform::Windows,
    },
    invite: invite.map(str::to_owned),
    jti: IdempotencyKey::generate().to_string(),
    iat,
    exp: iat + 300,
  };
  signing.sign_token(&claims).expect("token").expose().to_owned()
}

fn error(code: &str) -> Value {
  json!({"error": code, "message": code})
}

/// The request line, the headers in lowercase, and the body.
fn read_request(stream: &mut TcpStream) -> Option<(String, String, String)> {
  let mut buffer = Vec::new();
  let mut chunk = [0_u8; 4096];
  loop {
    let read = stream.read(&mut chunk).ok()?;
    if read == 0 {
      return None;
    }
    buffer.extend_from_slice(&chunk[..read]);
    let text = String::from_utf8_lossy(&buffer).into_owned();
    let Some((head, body)) = text.split_once("\r\n\r\n") else {
      continue;
    };
    let headers = head.to_ascii_lowercase();
    let length = headers
      .lines()
      .find_map(|line| line.strip_prefix("content-length:"))
      .and_then(|value| value.trim().parse::<usize>().ok())
      .unwrap_or(0);
    if body.len() >= length {
      let line = head.lines().next().unwrap_or_default().to_owned();
      return Some((line, headers, body.to_owned()));
    }
  }
}

fn now() -> i64 {
  let elapsed = SystemTime::now().duration_since(UNIX_EPOCH).expect("after 1970");
  i64::try_from(elapsed.as_secs()).expect("seconds fit")
}
