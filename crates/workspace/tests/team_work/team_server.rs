//! A fake team server on loopback with the team server's routes and shapes for authorized work (its
//! `http/workspace/mod.rs`, `bundles.rs`, `heartbeat.rs`, `docs/claim-authority.md`, `docs/module-branches.md`,
//! `docs/work-reports.md`): one project with one task in a module, under `/api/v2/projects/{p}`. The commands with
//! their receipts are in [`crate::team_commands`]; a work start names a schema-3 bundle, which carries the task's
//! number and the pinned versions' labels. Reads: `GET …/bundles/{sha256}` and `GET …/bundles/{sha256}/files/{file}`
//! for the claim's holder in its session (`409 claim_lost` otherwise), and `POST …/heartbeat` with D1's renewal rule.
//! Each token is one member's session, [`ADMIN`] a workspace Admin's. A test can take the server down (every
//! connection closed unanswered), make it lose the next write's answer or drop the next bundle file read, put the work
//! in a wait for a person, and turn the workspace's automatic merge on. A test binary may add routes of its own
//! ([`Extra`]), e.g. the reads its tools make.

#![allow(dead_code, reason = "each test binary uses some of the server")]

use std::{
  collections::HashMap,
  io::Write,
  net::{TcpListener, TcpStream},
  sync::Arc,
  thread,
};

use orchester_api::v2::{ExecutionBundle, Subject};
use orchester_core::id::{ClaimId, ModuleId, ProjectId, ServerId, TaskId, UserId, WorkId};
use parking_lot::Mutex;
use serde_json::{Value, json};

use crate::{
  fixture::Config,
  http::{Request, respond},
};

/// The person's session on this computer.
pub const MEMBER: &str = "member-token";
/// Another member's session, which takes the claim over.
pub const OTHER: &str = "other-token";
/// A workspace Admin's session, on their own computer.
pub const ADMIN: &str = "admin-token";
/// The module's branch, as the server assigned it (D3).
pub const MODULE_BRANCH: &str = "module/payments";

#[derive(Clone)]
pub struct TeamServer {
  pub url: String,
  pub server: ServerId,
  pub project: ProjectId,
  pub task: TaskId,
  pub module: ModuleId,
  state: Arc<Mutex<State>>,
}

/// A subject's current claim: its id and the session that holds it.
#[derive(Clone)]
pub struct Claim {
  pub id: ClaimId,
  pub token: String,
}

/// A work the server made under a claim.
#[derive(Clone)]
pub struct Work {
  pub id: WorkId,
  pub claim: ClaimId,
  pub bundle: String,
  pub state: String,
  pub revision: u64,
  pub runs: Vec<String>,
}

/// A delivery of the module's branch, as the server keeps it (`deliveries`): bound to the module's claim, its branch
/// and both commits, with the Admin's authorization or none (the automatic merge, or a request that waits for the
/// Admin); `waits` is whom a requested one waits for, as the server's schema 27 keeps it: `admin` for the Admin's
/// authorization, `holder` once the Admin authorized it and until the holder's next request takes the merge.
#[derive(Clone)]
pub struct Delivery {
  pub authorization: Option<String>,
  pub claim: ClaimId,
  pub branch: String,
  pub source_commit: String,
  pub base_commit: String,
  pub state: String,
  pub revision: u64,
  pub merge_commit: Option<String>,
  pub waits: Option<&'static str>,
}

pub struct State {
  pub config: Config,
  pub project: ProjectId,
  pub task: TaskId,
  pub module: ModuleId,
  pub claim: Option<Claim>,
  pub module_claim: Option<Claim>,
  pub works: Vec<Work>,
  /// By hash: the bundle as JSON, and the claim it was built for.
  pub bundles: HashMap<String, (Value, ClaimId)>,
  /// By token and operation id: the request (expected revisions and command) and the kept answer.
  pub operations: HashMap<(String, String), (Value, u16, Value)>,
  /// By event id: the accepted report's command and the answer it got.
  pub events: HashMap<String, (Value, Value)>,
  /// By work and run: the last accepted sequence.
  pub sequences: HashMap<(WorkId, String), u64>,
  /// Every accepted report's sequence and outcome kind, in the order the server accepted them.
  pub accepted: Vec<(u64, String)>,
  /// The module's deliveries, oldest first.
  pub deliveries: Vec<Delivery>,
  /// The workspace's Admin-owned automatic merge (D7).
  pub automatic_merge: bool,
  pub cursor: u64,
  pub down: bool,
  pub lose_next: bool,
  pub drop_next_file: bool,
  /// The `schema_version` the next work start's receipt names its bundle with, when it is not the bundle's own.
  pub receipt_schema: Option<u32>,
  /// What the next work start's bundle read answers instead of the bundle: JSON that is no bundle.
  pub bundle_body: Option<Value>,
  /// The `details` every work start is refused `bundle_missing` with, when set: `{}` or `{reason:{…}}`.
  pub bundle_missing: Option<Value>,
  /// The task is in no module: its start names the task's revision alone, and its bundle no module.
  pub in_no_module: bool,
  /// Whether the work waits for a person: the heartbeat answers `decision_required [D1]`.
  pub waits_for_person: bool,
  pub works_made: usize,
  pub heartbeats: usize,
  pub reports_sent: usize,
  /// The `delivery.request` and `delivery.report` commands carried out, not their repeats.
  pub delivery_requests: usize,
  pub delivery_reports: usize,
}

/// Routes a test binary adds to the server's own: asked first, without the state's lock, for every request of a known
/// session while the server is up; `true` when they answered it.
pub type Extra = Arc<dyn Fn(&Mutex<State>, &Request, &mut TcpStream) -> bool + Send + Sync>;

impl TeamServer {
  /// The server, its task claimed by nobody, its work built from `config`.
  pub fn start(config: Config) -> Self {
    Self::start_with(config, None)
  }

  /// [`Self::start`], with `extra` routes answered before the server's own.
  pub fn start_with(config: Config, extra: Option<Extra>) -> Self {
    let listener = TcpListener::bind("127.0.0.1:0").expect("listen");
    // By address: `localhost` tries IPv6 first, which costs every connection a few hundred milliseconds on Windows.
    let url = format!("http://{}", listener.local_addr().expect("address"));
    let (project, task, module) = (ProjectId::generate(), TaskId::generate(), ModuleId::generate());
    let state = Arc::new(Mutex::new(State {
      config,
      project,
      task,
      module,
      claim: None,
      module_claim: None,
      works: Vec::new(),
      bundles: HashMap::new(),
      operations: HashMap::new(),
      events: HashMap::new(),
      sequences: HashMap::new(),
      accepted: Vec::new(),
      deliveries: Vec::new(),
      automatic_merge: false,
      cursor: 10,
      down: false,
      lose_next: false,
      drop_next_file: false,
      receipt_schema: None,
      bundle_body: None,
      bundle_missing: None,
      in_no_module: false,
      waits_for_person: false,
      works_made: 0,
      heartbeats: 0,
      reports_sent: 0,
      delivery_requests: 0,
      delivery_reports: 0,
    }));
    let serving = state.clone();
    thread::spawn(move || {
      for stream in listener.incoming().flatten() {
        let (state, extra) = (serving.clone(), extra.clone());
        thread::spawn(move || answer(&state, extra.as_ref(), stream));
      }
    });
    Self {
      url,
      server: ServerId::generate(),
      project,
      task,
      module,
      state,
    }
  }

  pub fn state(&self) -> parking_lot::MutexGuard<'_, State> {
    self.state.lock()
  }

  /// Takes the server down, or brings it back: while down, every connection is closed unanswered.
  pub fn set_down(&self, down: bool) {
    self.state.lock().down = down;
  }

  /// The next write is carried out and its connection closed without an answer.
  pub fn lose_next_response(&self) {
    self.state.lock().lose_next = true;
  }

  /// The task is in no module from now on.
  pub fn task_in_no_module(&self) {
    self.state.lock().in_no_module = true;
  }

  /// The next bundle file read is closed without an answer.
  pub fn drop_next_file(&self) {
    self.state.lock().drop_next_file = true;
  }

  pub fn wait_for_person(&self, waits: bool) {
    self.state.lock().waits_for_person = waits;
  }
}

fn answer(shared: &Mutex<State>, extra: Option<&Extra>, mut stream: TcpStream) {
  let Some(request) = Request::read(&stream) else {
    return;
  };
  if shared.lock().down {
    return;
  }
  if ![MEMBER, OTHER, ADMIN].contains(&request.token.as_str()) {
    return respond(
      &mut stream,
      401,
      &json!({"error": "unauthorized", "message": "refused"}),
    );
  }
  if extra.is_some_and(|extra| extra(shared, &request, &mut stream)) {
    return;
  }
  let mut state = shared.lock();
  let prefix = format!("/api/v2/projects/{}", state.project);
  let Some(route) = request.path.strip_prefix(&prefix).map(str::to_owned) else {
    return respond(&mut stream, 404, &refusal(None, "not_found"));
  };
  let segments: Vec<&str> = route.split('/').filter(|segment| !segment.is_empty()).collect();
  let (status, body) = match (request.method.as_str(), segments.as_slice()) {
    ("POST", ["commands"]) => crate::team_commands::command(&mut state, &request),
    ("GET", ["operations", operation]) => crate::team_commands::operation_read(&state, &request.token, operation),
    ("GET", ["bundles", sha256]) => bundle_read(&state, &request.token, sha256),
    ("GET", ["bundles", sha256, "files", file]) => {
      if std::mem::take(&mut state.drop_next_file) {
        return;
      }
      match file_read(&state, &request.token, (sha256, file)) {
        Ok(bytes) => return respond_bytes(&mut stream, &bytes),
        Err(refused) => refused,
      }
    }
    ("POST", ["heartbeat"]) => heartbeat(&mut state, &request),
    _ => (404, refusal(None, "not_found")),
  };
  if status < 300 && request.method != "GET" && std::mem::take(&mut state.lose_next) {
    return;
  }
  drop(state);
  respond(&mut stream, status, &body);
}

/// `{operation_id?, code, details}`: the team-authority refusal.
pub fn refusal(operation: Option<&str>, code: &str) -> Value {
  match operation {
    Some(operation) => json!({"operation_id": operation, "code": code, "details": {}}),
    None => json!({"code": code, "details": {}}),
  }
}

pub fn person() -> Value {
  json!({"id": UserId::generate(), "display_name": "Ivan"})
}

/// The task's claim `id` as `ClaimRead` is.
pub fn claim_read(state: &State, id: ClaimId) -> Value {
  subject_claim_read(&json!({"kind": "task", "id": state.task}), id)
}

/// The claim `id` of `subject` as `ClaimRead` is.
pub fn subject_claim_read(subject: &Value, id: ClaimId) -> Value {
  json!({"subject": subject, "id": id, "holder": person(), "acquired_at": 1,
    "last_execution_heartbeat_at": 1, "stale_at": 900_001, "stale": false, "parent_module_claim_id": null})
}

/// Whether `token` holds the task's claim `claim` now, in its session.
pub fn holds(state: &State, token: &str, claim: ClaimId) -> bool {
  state
    .claim
    .as_ref()
    .is_some_and(|current| current.id == claim && current.token == token)
}

/// Whether `token` holds the module's claim `claim` now, in its session.
pub fn holds_module(state: &State, token: &str, claim: ClaimId) -> bool {
  state
    .module_claim
    .as_ref()
    .is_some_and(|current| current.id == claim && current.token == token)
}

fn bundle_read(state: &State, token: &str, sha256: &str) -> (u16, Value) {
  match state.bundles.get(sha256) {
    Some((bundle, claim)) if holds(state, token, *claim) => (200, bundle.clone()),
    _ => (409, refusal(None, "claim_lost")),
  }
}

fn file_read(state: &State, token: &str, (sha256, file): (&str, &str)) -> Result<Vec<u8>, (u16, Value)> {
  let Some((bundle, claim)) = state.bundles.get(sha256) else {
    return Err((409, refusal(None, "claim_lost")));
  };
  if !holds(state, token, *claim) {
    return Err((409, refusal(None, "claim_lost")));
  }
  let bundle: ExecutionBundle = serde_json::from_value(bundle.clone()).expect("a bundle");
  let pinned = bundle
    .inputs()
    .files()
    .iter()
    .any(|pinned| pinned.sha256.as_str() == file);
  if pinned && orchester_core::run::bundle::file_sha256(crate::fixture::NOTES).as_str() == file {
    Ok(crate::fixture::NOTES.to_vec())
  } else {
    Err((404, refusal(None, "not_found")))
  }
}

/// `POST …/heartbeat {authorities}`: renews the current ones while their work runs (D1).
fn heartbeat(state: &mut State, request: &Request) -> (u16, Value) {
  state.heartbeats += 1;
  let authorities = request.json()["authorities"].as_array().expect("authorities").clone();
  let (mut renewed, mut lost) = (Vec::new(), Vec::new());
  for authority in authorities {
    let claim: ClaimId = serde_json::from_value(authority["claim_id"].clone()).expect("a claim id");
    let work = authority["work_id"].as_str().map(str::to_owned);
    let latest = state.works.iter().rev().find(|known| known.claim == claim);
    let current = holds(state, &request.token, claim)
      && work
        .as_deref()
        .is_none_or(|work| latest.is_some_and(|latest| latest.id.to_string() == work));
    if !current {
      lost.push(authority);
      continue;
    }
    if state.waits_for_person {
      return (
        409,
        json!({"code": "decision_required", "details": {"decisions": ["D1"]}}),
      );
    }
    if latest.is_none_or(|latest| latest.state != "running") {
      return (409, refusal(None, "transition_denied"));
    }
    renewed.push(claim_read(state, claim));
  }
  (200, json!({"renewed": renewed, "lost": lost}))
}

/// The bundle of work started under `claim` from `base_commit`, kept by its hash.
pub fn build_bundle(state: &mut State, claim: ClaimId, base_commit: &str) -> Value {
  let subject = Subject::Task { id: state.task };
  let module = (!state.in_no_module).then_some(state.module);
  let bundle = state
    .config
    .bundle((state.project, subject), claim, module, base_commit);
  let mut reference = bundle.bundle_ref().expect("a hash");
  if let Some(schema) = state.receipt_schema.take() {
    reference.schema_version = schema;
  }
  let wire = match state.bundle_body.take() {
    Some(body) => body,
    None => serde_json::to_value(&bundle).expect("a bundle"),
  };
  state
    .bundles
    .insert(reference.sha256.as_str().to_owned(), (wire, claim));
  serde_json::to_value(reference).expect("a bundle reference")
}

fn respond_bytes(stream: &mut TcpStream, bytes: &[u8]) {
  let head = format!(
    "HTTP/1.1 200 Answer\r\nContent-Type: application/octet-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
    bytes.len()
  );
  // A client that hung up learns nothing; the test reads what reached it.
  let _ = stream.write_all(head.as_bytes()).and_then(|()| stream.write_all(bytes));
}
