//! A fake team server on loopback with the team server's routes and shapes (`docs/cloud-api.md`, "Team authority P2
//! commands"): one project with its tasks and workspace policy, an Admin's and a member's tokens. The commands with
//! their receipts are in [`crate::commands`]. The definition routes keep the server's rules: `PATCH
//! /api/v1/projects/{p}/tasks/{t}` is the Admin's (`403 not_admin` for a member) on the task's revision
//! (`409 revision_conflict` with the current task), and `PATCH /api/v2/projects/{p}/policy` on the policy's revision
//! (`409 policy_revision_conflict` with the current policy). A test makes the server carry out the next write and hang
//! up without answering, and chooses the id of the next task it creates.

use std::{
  collections::HashMap,
  net::{TcpListener, TcpStream},
  sync::Arc,
  thread,
};

use orchester_core::id::{ProjectId, ServerId, TaskId, UserId};
use parking_lot::Mutex;
use serde_json::{Value, json};

use crate::http::{Request, respond};

pub const ADMIN: &str = "admin-token";
pub const MEMBER: &str = "member-token";

#[derive(Clone)]
pub struct TeamServer {
  pub url: String,
  pub server: ServerId,
  pub project: ProjectId,
  state: Arc<Mutex<State>>,
}

#[derive(Clone)]
pub struct Task {
  pub id: TaskId,
  pub number: u32,
  pub title: String,
  /// The team pipeline it names, by its id.
  pub pipeline: Option<String>,
  /// Its workflow status as the wire names it.
  pub workflow: String,
  pub revision: u64,
}

#[derive(Default)]
pub struct State {
  pub tasks: Vec<Task>,
  /// The workspace policy: its revision and automatic merge.
  pub policy: (u64, bool),
  /// By token and operation id: the request (expected revisions and command) and the kept answer.
  pub operations: HashMap<(String, String), (Value, u16, Value)>,
  pub cursor: u64,
  lose_next: bool,
  pub next_task: Option<TaskId>,
  /// A team pipeline archived on the server, by its id: a task moved to it is `422 invalid`, `pipeline_archived`.
  pub archived_pipeline: Option<String>,
  /// Every request to `POST …/commands`, and every write a definition route carried out.
  commands: usize,
  pub created: usize,
  writes: usize,
}

impl TeamServer {
  pub fn start() -> Self {
    let listener = TcpListener::bind("127.0.0.1:0").expect("listen");
    // By address: `localhost` tries IPv6 first, which costs every connection a few hundred milliseconds on Windows.
    let url = format!("http://{}", listener.local_addr().expect("address"));
    let state = Arc::new(Mutex::new(State {
      cursor: 10,
      policy: (1, false),
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
    Self {
      url,
      server: ServerId::generate(),
      project,
      state,
    }
  }

  /// A task the project has, at revision 1.
  pub fn add_task(&self, title: &str) -> Task {
    let mut state = self.state.lock();
    let task = Task {
      id: TaskId::generate(),
      number: u32::try_from(state.tasks.len()).expect("a few tasks") + 1,
      title: title.to_owned(),
      pipeline: None,
      workflow: "backlog".to_owned(),
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

  /// Moves `task` to `workflow` as another person would, raising its revision.
  pub fn move_to(&self, task: TaskId, workflow: &str) {
    let mut state = self.state.lock();
    let task = state.tasks.iter_mut().find(|known| known.id == task).expect("a task");
    workflow.clone_into(&mut task.workflow);
    task.revision += 1;
  }

  pub fn task(&self, id: TaskId) -> Task {
    let state = self.state.lock();
    state.tasks.iter().find(|task| task.id == id).expect("a task").clone()
  }

  /// Turns the automatic merge on or off as another Admin would, raising the policy's revision.
  pub fn set_policy(&self, automatic_merge: bool) {
    let mut state = self.state.lock();
    state.policy = (state.policy.0 + 1, automatic_merge);
  }

  pub fn policy(&self) -> (u64, bool) {
    self.state.lock().policy
  }

  /// The next write is carried out and its connection closed without an answer.
  pub fn lose_next_response(&self) {
    self.state.lock().lose_next = true;
  }

  pub fn next_task_id(&self, id: TaskId) {
    self.state.lock().next_task = Some(id);
  }

  /// The team pipeline `id` is archived here from now on.
  pub fn archive_pipeline(&self, id: &str) {
    self.state.lock().archived_pipeline = Some(id.to_owned());
  }

  /// How many command requests reached the server, and how many tasks they created.
  pub fn counts(&self) -> (usize, usize) {
    let state = self.state.lock();
    (state.commands, state.created)
  }

  /// How many writes the definition routes carried out.
  pub fn writes(&self) -> usize {
    self.state.lock().writes
  }
}

fn answer(state: &Mutex<State>, project: ProjectId, mut stream: TcpStream) {
  let Some(request) = Request::read(&stream) else {
    return;
  };
  if request.token != ADMIN && request.token != MEMBER {
    return respond(&mut stream, 401, &api_error("unauthorized"));
  }
  let (version, route) = route(&request.path, project);
  let segments: Vec<&str> = route.split('/').filter(|segment| !segment.is_empty()).collect();
  let mut state = state.lock();
  let (status, body) = match (version, request.method.as_str(), segments.as_slice()) {
    ("v2", "POST", ["commands"]) => {
      state.commands += 1;
      crate::commands::command(&mut state, &request)
    }
    ("v2", "GET", ["operations", operation]) => crate::commands::operation_read(&state, &request.token, operation),
    ("v2", "GET", ["policy"]) => (200, policy_response(&state, &request.token)),
    ("v2", "PATCH", ["policy"]) => change_policy(&mut state, &request),
    ("v1", "GET", ["tasks", task]) => (200, json!({"task": shared_task(find(&state, task))})),
    ("v1", "PATCH", ["tasks", task]) => patch_task(&mut state, &request, task),
    _ => (404, api_error("not_found")),
  };
  if status < 300 && request.method != "GET" && std::mem::take(&mut state.lose_next) {
    return;
  }
  drop(state);
  respond(&mut stream, status, &body);
}

/// The API version and the route under the project.
fn route(path: &str, project: ProjectId) -> (&'static str, String) {
  for version in ["v1", "v2"] {
    if let Some(route) = path.strip_prefix(&format!("/api/{version}/projects/{project}")) {
      return (version, route.to_owned());
    }
  }
  ("", String::new())
}

fn find<'a>(state: &'a State, task: &str) -> &'a Task {
  let id: TaskId = task.parse().expect("a task id");
  state.tasks.iter().find(|known| known.id == id).expect("a task")
}

/// `PATCH /api/v1/projects/{p}/tasks/{t}`: `{revision, changes}`, the Admin's.
fn patch_task(state: &mut State, request: &Request, task: &str) -> (u16, Value) {
  if request.token != ADMIN {
    return (403, api_error("not_admin"));
  }
  let body = request.json();
  let current = find(state, task).clone();
  if body["revision"].as_u64() != Some(current.revision) {
    let mut conflict = api_error("revision_conflict");
    conflict["current"] = shared_task(&current);
    return (409, conflict);
  }
  if body["changes"]["pipeline"]
    .as_str()
    .is_some_and(|named| state.archived_pipeline.as_deref() == Some(named))
  {
    let mut refused = api_error("invalid");
    refused["reason"] = json!("pipeline_archived");
    return (422, refused);
  }
  let task = state
    .tasks
    .iter_mut()
    .find(|known| known.id == current.id)
    .expect("a task");
  if let Some(title) = body["changes"]["title"].as_str() {
    title.clone_into(&mut task.title);
  }
  if let Some(workflow) = body["changes"]["workflow"].as_str() {
    workflow.clone_into(&mut task.workflow);
  }
  if let Some(pipeline) = body["changes"].get("pipeline") {
    task.pipeline = pipeline.as_str().map(str::to_owned);
  }
  task.revision += 1;
  let answer = json!({"task": shared_task(task)});
  state.writes += 1;
  (200, answer)
}

/// `PATCH /api/v2/projects/{p}/policy`: `{revision, automatic_merge}`, the Admin's.
fn change_policy(state: &mut State, request: &Request) -> (u16, Value) {
  if request.token != ADMIN {
    return (403, api_error("forbidden"));
  }
  let body = request.json();
  let (revision, automatic_merge) = state.policy;
  if body["revision"].as_u64() != Some(revision) {
    return (
      409,
      json!({"error": "policy_revision_conflict", "current": {"revision": revision, "automatic_merge": automatic_merge}}),
    );
  }
  state.policy = (revision + 1, body["automatic_merge"].as_bool().expect("a setting"));
  state.writes += 1;
  (200, policy_response(state, &request.token))
}

fn policy_response(state: &State, token: &str) -> Value {
  let (revision, automatic_merge) = state.policy;
  json!({
    "policy": {"revision": revision, "automatic_merge": automatic_merge},
    "capabilities": {"read_policy": true, "change_policy": token == ADMIN}
  })
}

/// The `/api/v1` refusal: `{error, message}`.
pub fn api_error(code: &str) -> Value {
  json!({"error": code, "message": "refused"})
}

pub fn person() -> Value {
  json!({"id": UserId::generate(), "display_name": "Alice"})
}

/// A task as the server's `SharedTask` is.
pub fn shared_task(task: &Task) -> Value {
  let attribution = json!({"by": person(), "at": 1});
  json!({
    "id": task.id, "number": task.number, "key": format!("KON-{}", task.number), "former_key": null,
    "revision": task.revision, "title": task.title, "brief": "", "criteria": [], "pipeline": task.pipeline, "limits": null,
    "workflow": task.workflow, "priority": "none", "tags": [], "blocking": [], "blocked_reason": null, "module": null,
    "status": "draft", "has_run": false, "created": attribution, "updated": attribution, "claim": null, "run": null
  })
}
