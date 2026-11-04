//! A fake team server on loopback with the team server's routes and shapes for authorized work (its
//! `http/workspace/mod.rs`, `bundles.rs`, `heartbeat.rs`, `docs/claim-authority.md`, `docs/module-branches.md`,
//! `docs/work-reports.md`): one project with one task in a module, under `/api/v2/projects/{p}`. The commands with
//! their receipts are in [`crate::team_commands`]. Reads: `GET …/tasks/{task}` (the task's number), the library
//! (`POST …/library`), `GET …/bundles/{sha256}` and `GET …/bundles/{sha256}/files/{file}` for the claim's holder in its
//! session
//! (`409 claim_lost` otherwise), and `POST …/heartbeat` with D1's renewal rule. Each token is one member's session. A
//! test can take the server down (every connection closed unanswered), make it lose the next write's answer or drop
//! the next bundle file read, and put the work in a wait for a person.

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
  fixture::{Config, TASK_NUMBER},
  http::{Request, respond},
};

/// The person's session on this computer.
pub const MEMBER: &str = "member-token";
/// Another member's session, which takes the claim over.
pub const OTHER: &str = "other-token";

#[derive(Clone)]
pub struct TeamServer {
  pub url: String,
  pub server: ServerId,
  pub project: ProjectId,
  pub task: TaskId,
  pub module: ModuleId,
  state: Arc<Mutex<State>>,
}

/// The task's current claim: its id and the session that holds it.
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

pub struct State {
  pub config: Config,
  pub project: ProjectId,
  pub task: TaskId,
  pub module: ModuleId,
  pub claim: Option<Claim>,
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
  pub cursor: u64,
  pub down: bool,
  pub lose_next: bool,
  pub drop_next_file: bool,
  /// Whether the work waits for a person: the heartbeat answers `decision_required [D1]`.
  pub waits_for_person: bool,
  pub works_made: usize,
  pub heartbeats: usize,
  pub reports_sent: usize,
}

impl TeamServer {
  /// The server, its task claimed by nobody, its work built from `config`.
  pub fn start(config: Config) -> Self {
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
      works: Vec::new(),
      bundles: HashMap::new(),
      operations: HashMap::new(),
      events: HashMap::new(),
      sequences: HashMap::new(),
      accepted: Vec::new(),
      cursor: 10,
      down: false,
      lose_next: false,
      drop_next_file: false,
      waits_for_person: false,
      works_made: 0,
      heartbeats: 0,
      reports_sent: 0,
    }));
    let serving = state.clone();
    thread::spawn(move || {
      for stream in listener.incoming().flatten() {
        let state = serving.clone();
        thread::spawn(move || answer(&state, stream));
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

  /// The next bundle file read is closed without an answer.
  pub fn drop_next_file(&self) {
    self.state.lock().drop_next_file = true;
  }

  pub fn wait_for_person(&self, waits: bool) {
    self.state.lock().waits_for_person = waits;
  }
}

fn answer(state: &Mutex<State>, mut stream: TcpStream) {
  let Some(request) = Request::read(&stream) else {
    return;
  };
  let mut state = state.lock();
  if state.down {
    return;
  }
  if request.token != MEMBER && request.token != OTHER {
    return respond(
      &mut stream,
      401,
      &json!({"error": "unauthorized", "message": "refused"}),
    );
  }
  let prefix = format!("/api/v2/projects/{}", state.project);
  let Some(route) = request.path.strip_prefix(&prefix).map(str::to_owned) else {
    return respond(&mut stream, 404, &refusal(None, "not_found"));
  };
  let segments: Vec<&str> = route.split('/').filter(|segment| !segment.is_empty()).collect();
  let (status, body) = match (request.method.as_str(), segments.as_slice()) {
    ("POST", ["commands"]) => crate::team_commands::command(&mut state, &request),
    ("GET", ["operations", operation]) => crate::team_commands::operation_read(&state, &request.token, operation),
    ("GET", ["tasks", task]) if *task == state.task.to_string() => (200, task_read(&state)),
    ("POST", ["library"]) => (
      200,
      json!({"items": state.config.library(), "next": null, "cursor": "1"}),
    ),
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

/// The task as `GET …/tasks/{task}` reads it.
fn task_read(state: &State) -> Value {
  let attribution = json!({"by": person(), "at": 1});
  json!({
    "id": state.task, "number": TASK_NUMBER, "key": format!("KON-{TASK_NUMBER}"), "former_key": null, "revision": 1,
    "title": "Say hello", "brief": "Append one line.", "criteria": ["README.md ends with hello"], "pipeline": null,
    "limits": null, "workflow": "todo", "priority": "none", "tags": [], "blocking": [], "blocked_reason": null,
    "module": state.module, "status": "queued", "has_run": false, "created": attribution, "updated": attribution,
    "claim": null, "run": null, "assignee": null, "work": null, "effective_claim": null,
    "capabilities": {"project_id": state.project, "revision": 1, "role": "member", "actions": [], "unresolved": []}
  })
}

/// The claim `id` as `ClaimRead` is.
pub fn claim_read(state: &State, id: ClaimId) -> Value {
  json!({"subject": {"kind": "task", "id": state.task}, "id": id, "holder": person(), "acquired_at": 1,
    "last_execution_heartbeat_at": 1, "stale_at": 900_001, "stale": false, "parent_module_claim_id": null})
}

/// Whether `token` holds claim `claim` now, in its session.
pub fn holds(state: &State, token: &str, claim: ClaimId) -> bool {
  state
    .claim
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
  let bundle = state
    .config
    .bundle((state.project, subject), claim, state.module, base_commit);
  let sha256 = bundle.sha256().expect("a hash").as_str().to_owned();
  let wire = serde_json::to_value(&bundle).expect("a bundle");
  state.bundles.insert(sha256.clone(), (wire, claim));
  json!({"sha256": sha256, "schema_version": 1})
}

fn respond_bytes(stream: &mut TcpStream, bytes: &[u8]) {
  let head = format!(
    "HTTP/1.1 200 Answer\r\nContent-Type: application/octet-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
    bytes.len()
  );
  // A client that hung up learns nothing; the test reads what reached it.
  let _ = stream.write_all(head.as_bytes()).and_then(|()| stream.write_all(bytes));
}
