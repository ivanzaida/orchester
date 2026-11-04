//! A fake team server on loopback for the S4 commands: one project with its tasks, an Admin's and a member's tokens,
//! and every operation's receipt or refusal kept by account and operation id with its request, so the same request
//! under the same id answers the same receipt and another request under it is `operation_mismatch`. A member's
//! definition write is `forbidden`, a stale expected revision `revision_conflict`. A test makes the server commit the
//! next command and hang up without answering, and chooses the id of the next task it creates.

use std::{
  collections::HashMap,
  net::{TcpListener, TcpStream},
  sync::Arc,
  thread,
};

use orchester_core::id::{ClaimId, ProjectId, TaskId, UserId, WorkId};
use parking_lot::Mutex;
use serde_json::{Value, json};

use crate::http::{Request, respond};

pub const ADMIN: &str = "admin-token";
pub const MEMBER: &str = "member-token";

#[derive(Clone)]
pub struct TeamServer {
  pub url: String,
  pub project: ProjectId,
  state: Arc<Mutex<State>>,
}

#[derive(Clone)]
pub struct Task {
  pub id: TaskId,
  pub number: u32,
  pub title: String,
  pub revision: u64,
}

#[derive(Default)]
struct State {
  tasks: Vec<Task>,
  /// By token and operation id: the request (expected revisions and command) and the kept answer.
  operations: HashMap<(String, String), (Value, u16, Value)>,
  cursor: u64,
  lose_next: bool,
  next_task: Option<TaskId>,
  commands: usize,
  created: usize,
}

impl TeamServer {
  pub fn start() -> Self {
    let listener = TcpListener::bind("127.0.0.1:0").expect("listen");
    // By address: `localhost` tries IPv6 first, which costs every connection a few hundred milliseconds on Windows.
    let url = format!("http://{}", listener.local_addr().expect("address"));
    let state = Arc::new(Mutex::new(State {
      cursor: 10,
      ..State::default()
    }));
    let project = ProjectId::generate();
    let serving = state.clone();
    thread::spawn(move || {
      for stream in listener.incoming().flatten() {
        let state = serving.clone();
        thread::spawn(move || answer(&state, project, stream));
      }
    });
    Self { url, project, state }
  }

  /// A task the project has, at revision 1.
  pub fn add_task(&self, title: &str) -> Task {
    let mut state = self.state.lock();
    let task = Task {
      id: TaskId::generate(),
      number: u32::try_from(state.tasks.len()).expect("a few tasks") + 1,
      title: title.to_owned(),
      revision: 1,
    };
    state.tasks.push(task.clone());
    task
  }

  /// Retitles `task` as another person would, raising its revision.
  pub fn retitle(&self, task: TaskId, title: &str) {
    let mut state = self.state.lock();
    let task = state.tasks.iter_mut().find(|known| known.id == task).expect("a task");
    title.clone_into(&mut task.title);
    task.revision += 1;
  }

  pub fn task(&self, id: TaskId) -> Task {
    let state = self.state.lock();
    state.tasks.iter().find(|task| task.id == id).expect("a task").clone()
  }

  /// The next command is committed and its connection closed without an answer.
  pub fn lose_next_response(&self) {
    self.state.lock().lose_next = true;
  }

  pub fn next_task_id(&self, id: TaskId) {
    self.state.lock().next_task = Some(id);
  }

  /// How many command requests reached the server, and how many tasks they created.
  pub fn counts(&self) -> (usize, usize) {
    let state = self.state.lock();
    (state.commands, state.created)
  }
}

fn answer(state: &Mutex<State>, project: ProjectId, mut stream: TcpStream) {
  let Some(request) = Request::read(&stream) else {
    return;
  };
  if request.token != ADMIN && request.token != MEMBER {
    return respond(&mut stream, 401, &refusal(None, "unauthenticated"));
  }
  let prefix = format!("/api/v2/projects/{project}");
  let route = request.path.strip_prefix(&prefix).unwrap_or_default().to_owned();
  let segments: Vec<&str> = route.split('/').filter(|segment| !segment.is_empty()).collect();
  match (request.method.as_str(), segments.as_slice()) {
    ("POST", ["commands"]) => command(state, project, &request, stream),
    ("GET", ["operations", operation]) => {
      let (status, body) = operation_read(&state.lock(), &request.token, operation);
      respond(&mut stream, status, &body);
    }
    ("GET", ["tasks", task]) => {
      let id: TaskId = task.parse().expect("a task id");
      let state = state.lock();
      let task = state.tasks.iter().find(|known| known.id == id).expect("a task");
      respond(&mut stream, 200, &task_read(project, task));
    }
    _ => respond(&mut stream, 404, &refusal(None, "not_found")),
  }
}

/// `POST …/commands`: the kept answer of an operation seen before, else the command carried out and its answer kept.
fn command(state: &Mutex<State>, project: ProjectId, request: &Request, mut stream: TcpStream) {
  let body = request.json();
  let operation = body["operation_id"].as_str().expect("an operation id").to_owned();
  let identity = json!({ "expected": body["expected"], "command": body["command"] });
  let mut state = state.lock();
  state.commands += 1;
  let key = (request.token.clone(), operation.clone());
  let (status, answer) = match state.operations.get(&key) {
    Some((known, status, answer)) if *known == identity => (*status, answer.clone()),
    Some(_) => (409, refusal(Some(&operation), "operation_mismatch")),
    None => {
      let (status, answer) = carry_out(&mut state, project, &request.token, &body);
      state.operations.insert(key, (identity, status, answer.clone()));
      (status, answer)
    }
  };
  if std::mem::take(&mut state.lose_next) {
    return;
  }
  drop(state);
  respond(&mut stream, status, &answer);
}

fn carry_out(state: &mut State, project: ProjectId, token: &str, body: &Value) -> (u16, Value) {
  let operation = body["operation_id"].as_str().expect("an operation id");
  let command = &body["command"];
  let kind = command["kind"].as_str().expect("a kind");
  if kind == "claim.acquire" {
    return committed(state, operation, &claim(command));
  }
  if token != ADMIN {
    return (403, refusal(Some(operation), "forbidden"));
  }
  match kind {
    "task.create" => {
      let task = Task {
        id: state.next_task.take().unwrap_or_else(TaskId::generate),
        number: u32::try_from(state.tasks.len()).expect("a few tasks") + 1,
        title: command["content"]["title"].as_str().expect("a title").to_owned(),
        revision: 1,
      };
      state.tasks.push(task.clone());
      state.created += 1;
      committed(
        state,
        operation,
        &json!({"kind": "task", "task": task_read(project, &task), "effects": []}),
      )
    }
    "task.patch" => patch(state, project, operation, body),
    _ => (422, refusal(Some(operation), "invalid")),
  }
}

fn patch(state: &mut State, project: ProjectId, operation: &str, body: &Value) -> (u16, Value) {
  let id: TaskId = body["command"]["task_id"]
    .as_str()
    .expect("a task id")
    .parse()
    .expect("a task id");
  let expected = body["expected"][0]["revision"].as_u64().expect("an expected revision");
  let task = state.tasks.iter_mut().find(|task| task.id == id).expect("a task");
  if task.revision != expected {
    return (409, refusal(Some(operation), "revision_conflict"));
  }
  for change in body["command"]["changes"].as_array().expect("changes") {
    if change["field"] == "title" {
      change["value"].as_str().expect("a title").clone_into(&mut task.title);
    }
  }
  task.revision += 1;
  let read = task_read(project, task);
  committed(state, operation, &json!({"kind": "task", "task": read, "effects": []}))
}

/// A claim of the command's subject, allowing this computer to start its work.
fn claim(command: &Value) -> Value {
  json!({
    "kind": "claim", "subject": command["subject"],
    "claim": {"subject": command["subject"], "id": ClaimId::generate(), "holder": person(), "acquired_at": 1,
      "last_execution_heartbeat_at": 1, "stale_at": 2, "stale": false, "parent_module_claim_id": null},
    "children": [],
    "effects": [{"kind": "start", "work_id": WorkId::generate(),
      "bundle": {"sha256": "a".repeat(64), "schema_version": 1}}]
  })
}

fn committed(state: &mut State, operation: &str, result: &Value) -> (u16, Value) {
  state.cursor += 1;
  let cursor = state.cursor.to_string();
  (
    200,
    json!({"operation_id": operation, "state": "committed", "result": result, "cursor": cursor}),
  )
}

/// `GET …/operations/{operation}`: the kept receipt or refusal, or `not_found`.
fn operation_read(state: &State, token: &str, operation: &str) -> (u16, Value) {
  match state.operations.get(&(token.to_owned(), operation.to_owned())) {
    Some((_, 200, receipt)) => (
      200,
      json!({"operation_id": operation, "state": "committed", "response": receipt}),
    ),
    Some((_, _, error)) => (
      200,
      json!({"operation_id": operation, "state": "rejected", "error": error}),
    ),
    None => (404, refusal(None, "not_found")),
  }
}

fn refusal(operation: Option<&str>, code: &str) -> Value {
  match operation {
    Some(operation) => json!({"operation_id": operation, "code": code, "details": {}}),
    None => json!({"code": code, "details": {}}),
  }
}

fn person() -> Value {
  json!({"id": UserId::generate(), "display_name": "Alice"})
}

pub fn task_read(project: ProjectId, task: &Task) -> Value {
  let attribution = json!({"by": person(), "at": 1});
  json!({
    "id": task.id, "number": task.number, "key": format!("KON-{}", task.number), "former_key": null,
    "revision": task.revision, "title": task.title, "brief": "", "criteria": [], "pipeline": null, "limits": null,
    "workflow": "backlog", "priority": "none", "tags": [], "blocking": [], "blocked_reason": null, "module": null,
    "status": "draft", "has_run": false, "created": attribution, "updated": attribution, "claim": null, "run": null,
    "assignee": null, "work": null, "effective_claim": null,
    "capabilities": {"project_id": project, "revision": 1, "role": "admin", "actions": [], "unresolved": []}
  })
}
