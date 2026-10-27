//! A stand-in for Orchester Cloud's account API on loopback, in `docs/cloud-api.md`'s shapes: sign-in and sign-out of
//! a device, the account (`GET /me`), the account's invites (list, decline, and an identity token that accepts one)
//! and team servers (the account's list and the registry's entry), the key set and identity tokens, all signed with
//! `orchester-api`'s identity keys. It can end the device's token with a reason, stop answering, answer the check of a
//! sign-in as something else would ([`CheckAnswer`]), and answer a sign-in slowly; [`UNCONFIRMED`] and [`PAUSED`] sign
//! in to an unconfirmed email and a paused one, [`OTHER_EMAIL`] to a second account. Bohdan's two-factor
//! authentication is in [`two_factor`]. The real cloud on PostgreSQL is in `crates/desktop-app/tests/orchester_cloud`.

#![allow(
  dead_code,
  reason = "shared by test binaries and a QA example, each of which uses a part"
)]

use std::{
  collections::HashMap,
  io::{Read, Write},
  net::{SocketAddr, TcpListener, TcpStream},
  sync::Arc,
  thread,
  time::{Duration, SystemTime, UNIX_EPOCH},
};

use orchester_api::{
  cloud::TokenPurpose,
  identity::{IdentityClaims, SigningKey},
};
use orchester_core::id::IdempotencyKey;

#[path = "check_answer.rs"]
mod check_answer;
#[allow(
  clippy::duplicate_mod,
  reason = "the tests' one cloud key, loaded by every support module that signs for it, whichever of them a test binary includes"
)]
#[path = "../server_support/test_cloud.rs"]
mod test_cloud;
#[path = "two_factor_cloud.rs"]
mod two_factor;
pub use check_answer::CheckAnswer;
use parking_lot::Mutex;
use serde_json::{Value, json};
#[allow(
  unused_imports,
  reason = "for the binaries that sign in with a code; the others leave them"
)]
pub use two_factor::{RECOVERY_CODES, SECRET};

const DAY: i64 = 24 * 60 * 60;
pub const ACCOUNT: &str = "0191c7c0-0000-7000-8000-000000000b0b";
pub const EMAIL: &str = "bohdan@example.com";
pub const PASSWORD: &str = "correct horse battery";
const TOKEN: &str = "orcc_0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
/// A second account, with the same password: another person on the same computer.
pub const OTHER_ACCOUNT: &str = "0191c7c0-0000-7000-8000-0000000a11ce";
pub const OTHER_EMAIL: &str = "olena@example.com";
const OTHER_TOKEN: &str = "orcc_fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210";
/// Signs in with any password as an email that is not confirmed yet: `403 email_unconfirmed`.
pub const UNCONFIRMED: &str = "unconfirmed@example.com";
/// Signs in as an email whose sign-in is paused: `429 rate_limited`, for [`PAUSED_SECONDS`].
pub const PAUSED: &str = "paused@example.com";
pub const PAUSED_SECONDS: u64 = 900;

/// What the cloud holds: the team server it names, the invites waiting, and what was asked of it.
#[derive(Default)]
struct State {
  /// `{ id, name, address, fingerprint }` of the team server.
  server: Option<Value>,
  /// Invites, as `GET /invites` lists them, with the invitee's `answer` once there is one.
  invites: Vec<Value>,
  declined: Vec<String>,
  /// The invites that the tokens issued so far accept, one entry per token.
  accepting: Vec<String>,
  signed_out: bool,
  /// Bohdan's two-factor authentication.
  factors: two_factor::Factors,
  /// The tokens that ended: the reason each one's `401` names, if any, and when (milliseconds).
  ended: HashMap<&'static str, (Option<String>, i64)>,
  /// Connections are closed without an answer, as when the cloud cannot be reached.
  down: bool,
  /// How long a sign-in waits before it answers.
  sign_in_delay: Duration,
  /// Checks of a sign-in (`GET /me`) wait to be answered until they are let go.
  checks_held: bool,
  /// How many checks arrived, answered or not.
  checks_arrived: usize,
  /// Lookups of a server in the registry (`GET /servers/{id}`) wait to be answered until they are let go.
  registry_held: bool,
  /// How many lookups in the registry arrived, answered or not.
  registry_arrived: usize,
  /// How many times `GET /me` was asked.
  checks: usize,
  /// How `GET /me` is answered instead of the account.
  check_answer: Option<CheckAnswer>,
}

pub struct AccountCloud {
  pub address: SocketAddr,
  pub root_key: String,
  pub key_set: String,
  state: Arc<Mutex<State>>,
}

impl AccountCloud {
  pub fn start() -> Self {
    Self::start_on(0)
  }

  /// [`Self::start`] on loopback port `port`, `0` for one the system picks: a QA build that names the stand-in as its
  /// own cloud (`ORCHESTER_CLOUD_URL`) finds it there.
  pub fn start_on(port: u16) -> Self {
    // The tests' one cloud key (`server_support::test_cloud`), so a server that pinned it for codes takes these tokens.
    let root = test_cloud::root();
    let signing = Arc::new(test_cloud::signing());
    let key_set = test_cloud::key_set(now());
    let listener = TcpListener::bind(("127.0.0.1", port)).expect("listen");
    let address = listener.local_addr().expect("address");
    let state = Arc::new(Mutex::new(State::default()));
    let serving = (state.clone(), key_set.clone());
    thread::spawn(move || {
      for stream in listener.incoming().flatten() {
        // Each request on its own thread: a slow one does not hold the others.
        let (state, key_set, signing) = (serving.0.clone(), serving.1.clone(), signing.clone());
        thread::spawn(move || serve(stream, &state, &key_set, &signing));
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
    let server = json!({"id": server["id"], "name": server["name"], "address": server["address"]});
    state.invites.push(json!({
      "id": invite, "server": server, "kind": if project.is_some() { "project" } else { "server" },
      "project": project, "role": project.as_ref().map(|_| "member"), "inviter_display_name": "Alice Moreau",
      "expires_at": now + 7 * DAY * 1000,
    }));
  }

  pub fn declined(&self) -> Vec<String> {
    self.state.lock().declined.clone()
  }

  /// How many tokens accepting `invite` were issued.
  pub fn tokens_for(&self, invite: &str) -> usize {
    self.state.lock().accepting.iter().filter(|id| *id == invite).count()
  }

  /// From now on Bohdan has two-factor authentication off, as Olena does.
  pub fn two_factor_off(&self) {
    self.state.lock().factors.enabled = false;
  }

  /// From now on Bohdan has two-factor authentication on again, as when he turned it on in the browser.
  pub fn two_factor_on(&self) {
    self.state.lock().factors.enabled = true;
  }

  /// From now on Bohdan's password answers a challenge, which a code finishes.
  pub fn require_codes(&self) {
    self.state.lock().factors.codes_at_sign_in = true;
  }

  /// This computer signed in without a code, as before Bohdan turned two-factor authentication on.
  pub fn session_without_two_factor(&self) {
    self.state.lock().factors.this_session = false;
  }

  /// Every challenge given so far ends, as when its five minutes passed.
  pub fn end_challenges(&self) {
    self.state.lock().factors.end_challenges();
  }

  /// Code checks answer `429` while `limited`.
  pub fn limit_codes(&self, limited: bool) {
    self.state.lock().factors.limited = limited;
  }

  /// The first `count` of Bohdan's recovery codes were used before.
  pub fn use_recovery_codes(&self, count: usize) {
    self.state.lock().factors.use_recovery(count);
  }

  /// The code Bohdan's authenticator app shows that the stand-in takes next.
  pub fn next_code(&self) -> String {
    self.state.lock().factors.next_code(now())
  }

  pub fn signed_out(&self) -> bool {
    self.state.lock().signed_out
  }

  /// Ends the devices' tokens: from now on their requests answer `401` with `reason` (`revoked`, `password_reset`,
  /// `account_deleted`, `signed_out`, or none) and the time `at` (milliseconds), until that account signs in again.
  pub fn end_token(&self, reason: Option<&str>, at: i64) {
    let mut state = self.state.lock();
    for account in &ACCOUNTS {
      state.ended.insert(account.token, (reason.map(str::to_owned), at));
    }
  }

  /// Accepts the devices' tokens again, as after a new sign-in.
  pub fn accept_token(&self) {
    self.state.lock().ended.clear();
  }

  /// Whether the cloud answers: down, every connection is closed at once.
  pub fn set_down(&self, down: bool) {
    self.state.lock().down = down;
  }

  /// Sign-ins answer after `delay`.
  pub fn slow_sign_in(&self, delay: Duration) {
    self.state.lock().sign_in_delay = delay;
  }

  /// Holds the answers to checks of a sign-in (`GET /me`) until [`Self::let_checks_go`]; they answer with what holds
  /// then.
  pub fn hold_checks(&self) {
    self.state.lock().checks_held = true;
  }

  pub fn let_checks_go(&self) {
    self.state.lock().checks_held = false;
  }

  /// Holds the answers to lookups in the registry (`GET /servers/{id}`) until [`Self::let_registry_go`].
  pub fn hold_registry(&self) {
    self.state.lock().registry_held = true;
  }

  pub fn let_registry_go(&self) {
    self.state.lock().registry_held = false;
  }

  /// How many lookups in the registry arrived, held or answered.
  pub fn registry_arrived(&self) -> usize {
    self.state.lock().registry_arrived
  }

  /// How many checks of a sign-in arrived, held or answered.
  pub fn checks_arrived(&self) -> usize {
    self.state.lock().checks_arrived
  }

  /// Answers the check of a sign-in (`GET /me`) as `answer` says from now on; `None` with the account again.
  pub fn answer_checks(&self, answer: Option<CheckAnswer>) {
    self.state.lock().check_answer = answer;
  }

  /// How many times the account was asked for (`GET /me`).
  pub fn checks(&self) -> usize {
    self.state.lock().checks
  }
}

fn serve(mut stream: TcpStream, state: &Mutex<State>, key_set: &str, signing: &SigningKey) {
  if state.lock().down {
    return;
  }
  let Some((line, headers, body)) = read_request(&mut stream) else {
    return;
  };
  // The API at `/api/v1` on the web address, or at `/v1` on an API address of its own: the stand-in is both.
  let path = line
    .split_whitespace()
    .take(2)
    .collect::<Vec<_>>()
    .join(" ")
    .replacen(" /v1/", " /api/v1/", 1);
  if path == "GET /api/v1/me" {
    state.lock().checks_arrived += 1;
    while state.lock().checks_held {
      thread::sleep(Duration::from_millis(10));
    }
  }
  if path.starts_with("GET /api/v1/servers/") {
    state.lock().registry_arrived += 1;
    while state.lock().registry_held {
      thread::sleep(Duration::from_millis(10));
    }
  }
  let holder = ACCOUNTS
    .iter()
    .find(|account| headers.contains(&format!("authorization: bearer {}", account.token)));
  let ended = holder.and_then(|account| state.lock().ended.get(account.token).cloned());
  let check_answer = state.lock().check_answer;
  if let (Some(answer), Some(_), "GET /api/v1/me") = (check_answer, holder, path.as_str()) {
    // The test reads the answer or fails on its own; a broken connection needs nothing here.
    let _ = stream.write_all(answer.reply().as_bytes());
    return;
  }
  let (status, answer) = match ended {
    Some((reason, at)) => ended_token(reason, at),
    None => route(&path, (holder, &body), state, (key_set, signing)),
  };
  if path.starts_with("POST /api/v1/auth/sessions") {
    thread::sleep(state.lock().sign_in_delay);
    // A new sign-in is a new token: the ended one is forgotten.
    let token = answer.as_ref().and_then(|answer| answer["token"].as_str());
    if let Some(account) = ACCOUNTS.iter().find(|account| Some(account.token) == token) {
      state.lock().ended.remove(account.token);
    }
  }
  let retry = if status.starts_with("429") {
    format!("Retry-After: {PAUSED_SECONDS}\r\n")
  } else {
    String::new()
  };
  let answer = answer.map(|value| value.to_string()).unwrap_or_default();
  let reply = format!(
    "HTTP/1.1 {status}\r\nContent-Type: application/json\r\n{retry}Content-Length: {}\r\nConnection: close\r\n\r\n{answer}",
    answer.len()
  );
  // The test reads the answer or fails on its own; a broken connection needs nothing here.
  let _ = stream.write_all(reply.as_bytes());
}

type Answer = (&'static str, Option<Value>);

fn route(
  path: &str,
  (holder, body): (Option<&Account>, &str),
  state: &Mutex<State>,
  (key_set, signing): (&str, &SigningKey),
) -> Answer {
  let unauthorized = ("401 Unauthorized", Some(error("unauthorized")));
  let not_found = ("404 Not Found", Some(error("not_found")));
  match path {
    "GET /api/v1/keys" => ("200 OK", Some(json!({"key_set": key_set}))),
    "POST /api/v1/auth/sessions" => sign_in(body, state),
    "POST /api/v1/auth/sessions/two-factor" => finish_sign_in(body, state),
    _ if holder.is_none() => unauthorized,
    "GET /api/v1/me" => {
      let mut state = state.lock();
      state.checks += 1;
      let account = holder.map_or(Value::Null, Account::json);
      let two_factor = state.factors.status(holder.is_some_and(Account::is_bohdan));
      ("200 OK", Some(json!({ "account": account, "two_factor": two_factor })))
    }
    "POST /api/v1/identity-tokens" => holder
      .and_then(|holder| issue(body, (holder, state), signing))
      .unwrap_or(not_found),
    "DELETE /api/v1/auth/sessions/current" => {
      state.lock().signed_out = true;
      ("204 No Content", None)
    }
    "GET /api/v1/invites" => {
      let state = state.lock();
      let needs = !state.factors.session(holder.is_some_and(Account::is_bohdan));
      let open: Vec<Value> = state
        .invites
        .iter()
        .filter(|invite| invite.get("answer").is_none())
        .map(|invite| {
          let mut listed = invite.clone();
          listed["needs_two_factor"] = json!(needs);
          listed
        })
        .collect();
      ("200 OK", Some(json!({"invites": open})))
    }
    "GET /api/v1/servers" => {
      let servers: Vec<Value> = state
        .lock()
        .server
        .iter()
        .map(|server| {
          let mut listed = server.clone();
          listed["owner"] = json!(false);
          listed["joined_at"] = json!(now() * 1000);
          listed
        })
        .collect();
      ("200 OK", Some(json!({"servers": servers})))
    }
    _ => registered(path, state)
      .or_else(|| decline(path, state))
      .unwrap_or(not_found),
  }
}

/// `POST /api/v1/identity-tokens`: a token about `holder` for its purpose; with
/// `invite`, one that accepts that invite from the audience, open or accepted before, which is then accepted and
/// listed no more (`docs/cloud-api.md`). `None` for a declined one.
fn issue(body: &str, (holder, state): (&Account, &Mutex<State>), signing: &SigningKey) -> Option<Answer> {
  let request: Value = serde_json::from_str(body).unwrap_or(Value::Null);
  let audience = request["audience"].as_str().unwrap_or_default().to_owned();
  let invite = request["invite"].as_str();
  let purpose = serde_json::from_value(request["purpose"].clone()).ok()?;
  let mfa = state.lock().factors.session(holder.is_bohdan());
  if let Some(id) = invite {
    let mut state = state.lock();
    if let Some(refusal) = state.factors.refusal(holder.is_bohdan()) {
      // The invite stays open (`docs/cloud-api.md`, "What a two-factor session may do").
      return Some(("403 Forbidden", Some(error(refusal))));
    }
    let found = state.invites.iter_mut().find(|answered| {
      answered["id"] == id && answered["server"]["id"] == audience.as_str() && answered["answer"] != "declined"
    })?;
    found["answer"] = json!("accepted");
    state.accepting.push(id.to_owned());
  }
  let token = identity_token(signing, (holder, &audience, mfa), (invite, purpose));
  Some((
    "201 Created",
    Some(json!({"identity_token": token, "expires_at": now() + 300})),
  ))
}

/// `GET /api/v1/servers/{id}`: the registry's entry of the named server.
fn registered(path: &str, state: &Mutex<State>) -> Option<Answer> {
  let id = path.strip_prefix("GET /api/v1/servers/")?;
  let mut server = state.lock().server.clone().filter(|server| server["id"] == id)?;
  server["check_code"] = Value::Null;
  Some(("200 OK", Some(json!({"server": server}))))
}

/// The `401` of a token the cloud ended, with its reason when it has one.
fn ended_token(reason: Option<String>, at: i64) -> Answer {
  let mut refused = error("unauthorized");
  if let Some(reason) = reason {
    refused["reason"] = json!(reason);
    refused["ended_at"] = json!(at);
  }
  ("401 Unauthorized", Some(refused))
}

/// An account of the stand-in, with the token its sign-in gets.
struct Account {
  id: &'static str,
  email: &'static str,
  display_name: &'static str,
  token: &'static str,
}

impl Account {
  fn json(&self) -> Value {
    json!({"id": self.id, "email": self.email, "display_name": self.display_name, "created_at": 1})
  }

  /// Whether this is Bohdan, the account with two-factor authentication.
  fn is_bohdan(&self) -> bool {
    self.email == EMAIL
  }

  /// The answer of a sign-in of this account from `device`, as a password or a code finished it.
  fn signed_in(&self, device: &Value) -> Value {
    json!({"token": self.token, "device": device, "account": self.json()})
  }
}

const ACCOUNTS: [Account; 2] = [
  Account {
    id: ACCOUNT,
    email: EMAIL,
    display_name: "Bohdan Koval",
    token: TOKEN,
  },
  Account {
    id: OTHER_ACCOUNT,
    email: OTHER_EMAIL,
    display_name: "Olena Shevchuk",
    token: OTHER_TOKEN,
  },
];

fn sign_in(body: &str, state: &Mutex<State>) -> Answer {
  let request: Value = serde_json::from_str(body).unwrap_or(Value::Null);
  if request["email"] == UNCONFIRMED {
    return ("403 Forbidden", Some(error("email_unconfirmed")));
  }
  if request["email"] == PAUSED {
    return ("429 Too Many Requests", Some(error("rate_limited")));
  }
  let found = ACCOUNTS.iter().find(|account| request["email"] == account.email);
  let Some(account) = found.filter(|_| request["password"] == PASSWORD) else {
    return ("401 Unauthorized", Some(error("invalid_credentials")));
  };
  let device = json!({"id": "0191c7c0-0000-7000-8000-00000000d0d0", "name": request["device"]["name"],
    "platform": request["device"]["platform"], "created_at": 1, "last_used_at": 1, "current": true});
  if account.is_bohdan()
    && let Some(challenge) = state.lock().factors.challenge(&device, now() * 1000)
  {
    return ("202 Accepted", Some(challenge));
  }
  ("201 Created", Some(account.signed_in(&device)))
}

/// `POST /api/v1/auth/sessions/two-factor`: Bohdan's sign-in finished with a code.
fn finish_sign_in(body: &str, state: &Mutex<State>) -> Answer {
  let finished = state.lock().factors.finish(body, now() * 1000);
  match finished {
    Ok((device, left)) => {
      let mut answer = ACCOUNTS[0].signed_in(&device);
      answer["recovery_codes_left"] = json!(left);
      ("201 Created", Some(answer))
    }
    Err((status, code)) => (status, Some(error(code))),
  }
}

/// `POST /api/v1/invites/{id}/decline`; `None` for any other path or an invite not open.
fn decline(path: &str, state: &Mutex<State>) -> Option<Answer> {
  let id = path.strip_prefix("POST /api/v1/invites/")?.strip_suffix("/decline")?;
  let mut state = state.lock();
  let open = state
    .invites
    .iter_mut()
    .find(|invite| invite["id"] == id && invite.get("answer").is_none())?;
  open["answer"] = json!("declined");
  state.declined.push(id.to_owned());
  Some(("204 No Content", None))
}

fn identity_token(
  signing: &SigningKey,
  (holder, audience, mfa): (&Account, &str, bool),
  (invite, purpose): (Option<&str>, Option<TokenPurpose>),
) -> String {
  let iat = now();
  let claims = IdentityClaims {
    iss: "https://cloud.test".into(),
    aud: audience.into(),
    sub: holder.id.parse().expect("a cloud account id"),
    email: holder.email.into(),
    name: holder.display_name.into(),
    device: orchester_api::cloud::DeviceInfo {
      name: "Bohdan's laptop".into(),
      platform: orchester_api::cloud::CloudPlatform::Windows,
    },
    invite: invite.map(str::to_owned),
    purpose,
    mfa,
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
