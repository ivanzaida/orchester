//! The routes the tools read and change a team project with, added to the fake team server of the work tests
//! ([`crate::team_server`]) as the team server answers them: the S11 subscription (`ready`, then held open), the S10
//! task list, task, module list, module and a resource's versions, and a task's definition route, which refuses a
//! member `403 not_admin`, as `task.create` refuses one `403 forbidden`. Its one task, `KON-7`, is in its one module
//! and runs the team's pipeline, whose Builder profile runs on Codex with the publisher's MCP server `pencil-design`.

use std::{
  io::{Read, Write},
  net::TcpStream,
  sync::Arc,
};

use orchester_api::ReferenceTarget;
use orchester_core::{
  connection::AgentProvider,
  id::{ConnectionId, ResourceId, TaskId},
  resource::{AgentProfile, TeamAgentProfile},
};
use parking_lot::Mutex;
use serde_json::{Value, json};

use crate::{
  fixture::{self, Config},
  http::{Request, respond},
  team_server::{Extra, MEMBER, MODULE_BRANCH, State},
};

/// What the tools' reads and edits did on the server.
#[derive(Default)]
pub struct Seen {
  /// Every task definition edit that reached the server, refused or not.
  pub task_edits: usize,
  /// Every `task.create` that reached the server, refused or not.
  pub task_creates: usize,
  /// The pipeline the last task edit and the last `task.create` named, as sent.
  pub edited_pipeline: Value,
  /// The changes the last task edit sent, as sent.
  pub edited_changes: Value,
  pub created_pipeline: Value,
  /// The fields a test lays over the task the server reads (an object, or null for none): the records of who closed
  /// it or blocked it, as a server of this contract sends them.
  pub task_fields: Value,
  /// The team MCP server the Builder profile's version names, as the local server id a stopgap start looks it up by.
  pub publisher_mcp_server: Option<ConnectionId>,
  /// The number of the one task `KON-7` the server holds: 7, unless a test gives it another to collide with a local
  /// key.
  pub task_number: u32,
  /// Other tasks the server holds beside `KON-7`: the fields laid over a copy of it, including `id`, `number` and
  /// `key`.
  pub more_tasks: Vec<Value>,
  /// A task whose own route the server fails with `500`.
  pub broken_task: Option<TaskId>,
  /// How many tasks one page of the task list holds; 0 for all of them on one page.
  pub page_size: usize,
  /// How many `GET tasks/{id}` and task list requests the server answered.
  pub task_gets: usize,
  pub task_lists: usize,
  /// The server answers `GET capabilities` with `404`, as for a project the caller is no member of.
  pub no_member: bool,
}

/// The routes, and what they saw.
pub fn routes(config: &Config) -> (Extra, Arc<Mutex<Seen>>) {
  routes_running(config, Some(AgentProvider::CodexCli))
}

/// [`routes`] whose Builder profile version runs on `cli`, or is a body that names no CLI kind when `cli` is `None`.
pub fn routes_running(config: &Config, cli: Option<AgentProvider>) -> (Extra, Arc<Mutex<Seen>>) {
  let seen = Arc::new(Mutex::new(Seen::default()));
  let versions = Arc::new(Versions::of(config, cli));
  seen.lock().publisher_mcp_server = Some(versions.mcp_server);
  seen.lock().task_number = 7;
  let counted = seen.clone();
  let extra: Extra = Arc::new(move |state, request, stream| route(state, (&versions, &counted), request, stream));
  (extra, seen)
}

/// The team's published pipeline and profile, as their versions read.
struct Versions {
  pipeline: (ResourceId, Value),
  profile: (ResourceId, Value),
  mcp_server: ConnectionId,
}

impl Versions {
  /// The Builder profile names `cli` in the team's form; with no `cli` its body is the old local one, which names a
  /// connection and no CLI kind and cannot be read as a team profile.
  fn of(config: &Config, cli: Option<AgentProvider>) -> Self {
    let local = fixture::builder();
    let mcp_server = ConnectionId::generate();
    let team_mcp = ResourceId::from_uuid(mcp_server.as_uuid());
    let pipeline = fixture::one_step(config.profile, "Say hello.");
    let (body, references) = match cli {
      Some(cli) => {
        let profile = team_profile(local.body(), cli, team_mcp);
        (json!(profile), json!(ReferenceTarget::of_profile(&profile)))
      }
      None => (
        json!(local.body()),
        json!([{"type": "team_resource", "kind": "mcp_server", "id": team_mcp}]),
      ),
    };
    Self {
      mcp_server,
      pipeline: (
        config.pipeline,
        version(config.pipeline, &json!(pipeline.body()), &json!([])),
      ),
      profile: (config.profile, version(config.profile, &body, &references)),
    }
  }
}

/// `local` as the team holds it: running on `cli` and using the team MCP server `mcp_server`.
fn team_profile(local: &AgentProfile, cli: AgentProvider, mcp_server: ResourceId) -> TeamAgentProfile {
  TeamAgentProfile::new(
    local.role().clone(),
    local.responsibilities().to_owned(),
    local.instructions().clone(),
    (cli, local.model().clone(), local.settings().clone()),
  )
  .with_permissions(local.permissions().clone())
  .and_then(|profile| profile.with_mcp_servers(vec![mcp_server]))
  .expect("no writable folders, one MCP server")
}

fn version(resource: ResourceId, body: &Value, references: &Value) -> Value {
  json!({
    "resource": resource, "number": 1, "body": body, "hash": "a".repeat(64),
    "references": references, "based_on": null, "published": attribution(), "widens": false
  })
}

fn attribution() -> Value {
  json!({ "by": { "id": "01a183cf-b73c-71db-8a62-bd509770cd4e", "display_name": "Dana" }, "at": 1 })
}

fn capabilities(project: impl serde::Serialize) -> Value {
  json!({"project_id": project, "revision": 1, "role": "member",
    "actions": [{"action": "read", "subject": null, "allowed": true, "reason": null}], "unresolved": []})
}

/// Answers `request` when it is one of these routes.
fn route(
  state: &Mutex<State>,
  (versions, seen): (&Versions, &Mutex<Seen>),
  request: &Request,
  stream: &mut TcpStream,
) -> bool {
  let (project, task, module, pipeline) = {
    let state = state.lock();
    (state.project, state.task, state.module, state.config.pipeline)
  };
  let number = seen.lock().task_number;
  let path = request.path.split('?').next().unwrap_or_default();
  if let Some(route) = path.strip_prefix(&format!("/api/v1/projects/{project}/tasks/")) {
    let (status, body) = task_edit(seen, request, route);
    respond(stream, status, &body);
    return true;
  }
  let Some(route) = path.strip_prefix(&format!("/api/v2/projects/{project}")) else {
    return false;
  };
  let segments: Vec<&str> = route.split('/').filter(|segment| !segment.is_empty()).collect();
  let task_read = || {
    let mut read = json!({
      "id": task, "number": number, "key": format!("KON-{number}"), "former_key": null, "revision": 1, "title": "Say hello",
      "brief": "Append one line.", "criteria": ["README.md ends with hello"], "pipeline": pipeline.to_string(),
      "limits": null, "workflow": "todo", "priority": "medium", "tags": [], "blocking": [], "blocked_reason": null,
      "module": module, "release": null, "status": "queued", "has_run": false, "created": attribution(),
      "updated": attribution(), "claim": null, "run": null, "assignee": null, "work": null, "effective_claim": null,
      "capabilities": capabilities(project)
    });
    if let (Some(read), Some(fields)) = (read.as_object_mut(), seen.lock().task_fields.as_object()) {
      read.extend(fields.clone());
    }
    read
  };
  let summary = || {
    json!({
      "module": {
        "id": module, "name": "Webhooks", "description": "", "start": null, "target": null, "state": "open",
        "horizon": "later", "place": 0,
        "revision": 1, "created": attribution(), "updated": attribution(), "branch": MODULE_BRANCH,
        "assignee": null, "work": null, "effective_claim": null, "capabilities": capabilities(project),
        "pending_delivery": null
      },
      "progress": {"unit": "tasks", "scope": 1, "done": 0}, "tasks": 1,
      "forecast": {"remaining": 1, "history_days": 0, "history_delivered": 0, "outlook": {"kind": "not_enough_history"}}
    })
  };
  let page = |items: Vec<Value>| json!({"items": items, "next": null, "cursor": "10"});
  let (status, body) = match (request.method.as_str(), segments.as_slice()) {
    ("GET", ["subscription"]) => return subscription(stream),
    ("GET", ["capabilities"]) => member_capabilities(seen, project),
    (_, ["task-list"]) => (200, task_list(seen, request, (number, &task_read))),
    ("GET", ["tasks", id]) if seen.lock().broken_task.is_some_and(|broken| broken.to_string() == *id) => {
      seen.lock().task_gets += 1;
      (500, json!({"error": "internal", "message": "Broken"}))
    }
    ("GET", ["tasks", id]) if *id == task.to_string() => {
      seen.lock().task_gets += 1;
      (200, task_read())
    }
    ("GET", ["tasks", _]) => (404, json!({"error": "not_found", "message": "Not found"})),
    (_, ["module-list"]) => (200, page(vec![summary()])),
    ("GET", ["modules", id]) if *id == module.to_string() => (200, summary()),
    (_, ["resources", id, "version-list"]) => {
      let versions = [&versions.pipeline, &versions.profile];
      match versions.iter().find(|(resource, _)| resource.to_string() == *id) {
        Some((_, version)) => (200, page(vec![version.clone()])),
        None => (404, json!({"error": "not_found", "message": "Not found"})),
      }
    }
    ("POST", ["commands"]) if request.json()["command"]["kind"] == "task.create" => {
      let mut seen = seen.lock();
      seen.task_creates += 1;
      seen.created_pipeline = request.json()["command"]["content"]["pipeline"].clone();
      drop(seen);
      let operation = request.json()["operation_id"].clone();
      (
        403,
        json!({"operation_id": operation, "code": "forbidden", "details": {}}),
      )
    }
    _ => return false,
  };
  respond(stream, status, &body);
  true
}

/// A task's definition route, which refuses a member `403 not_admin`; what it was sent is seen.
fn task_edit(seen: &Mutex<Seen>, request: &Request, route: &str) -> (u16, Value) {
  let mut seen = seen.lock();
  seen.task_edits += 1;
  if request.method == "PATCH" {
    seen.edited_pipeline = request.json()["changes"]["pipeline"].clone();
    seen.edited_changes = request.json()["changes"].clone();
  }
  if request.token == MEMBER {
    (
      403,
      json!({"error": "not_admin", "message": "Only an admin changes this."}),
    )
  } else {
    (404, json!({"error": "not_found", "message": route}))
  }
}

/// `GET …/subscription` without `Last-Event-ID`: `ready` at the current cursor, then the stream stays open until the
/// client hangs up.
fn subscription(stream: &mut TcpStream) -> bool {
  let ready = json!({"kind": "ready", "cursor": "10"});
  let text = format!(
    "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n: connected\n\nid: 10\nevent: \
     ready\ndata: {ready}\n\n"
  );
  if stream.write_all(text.as_bytes()).and_then(|()| stream.flush()).is_err() {
    return true;
  }
  // The client sends nothing more: a read returns once it hung up.
  let mut rest = [0; 64];
  while matches!(stream.read(&mut rest), Ok(read) if read > 0) {}
  true
}

/// `POST …/task-list`: the tasks the request's module filter keeps, `KON-7` when its search finds it, then the more
/// tasks a test laid out, one page of `Seen::page_size` from the offset the cursor names.
fn task_list(seen: &Mutex<Seen>, request: &Request, (number, task_read): (u32, &dyn Fn() -> Value)) -> Value {
  seen.lock().task_lists += 1;
  let body = request.json();
  let search = body["filters"]["search"].as_str().map(str::to_lowercase);
  let found = search.is_none_or(|search| format!("kon-{number} say hello").contains(&search));
  let mut tasks = if found { vec![task_read()] } else { Vec::new() };
  let more_tasks = seen.lock().more_tasks.clone();
  for fields in more_tasks {
    let mut more = task_read();
    more
      .as_object_mut()
      .expect("a task")
      .extend(fields.as_object().expect("fields").clone());
    tasks.push(more);
  }
  let modules = body["filters"]["modules"].as_array().cloned().unwrap_or_default();
  tasks.retain(|task| modules.is_empty() || modules.contains(&task["module"]));
  let size = seen.lock().page_size;
  let from: usize = body["page"]["after"]
    .as_str()
    .map_or(0, |after| after.parse().expect("the cursor is an offset"));
  let size = if size == 0 { tasks.len() } else { size };
  let items: Vec<Value> = tasks.iter().skip(from).take(size).cloned().collect();
  let next = (from + size < tasks.len()).then(|| (from + size).to_string());
  json!({"items": items, "next": next, "cursor": "10"})
}

/// `GET …/capabilities`, which is `404` when a test made the caller no member of the project.
fn member_capabilities(seen: &Mutex<Seen>, project: impl serde::Serialize) -> (u16, Value) {
  if seen.lock().no_member {
    (404, json!({"error": "not_found", "message": "Not found"}))
  } else {
    (200, capabilities(project))
  }
}
