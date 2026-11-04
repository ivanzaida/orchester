//! The routes the tools read and change a team project with, added to the fake team server of the work tests
//! ([`crate::team_server`]) as the team server answers them: the S11 subscription (`ready`, then held open), the S10
//! task list, task, module list, module and a resource's versions, and a task's definition route, which refuses a
//! member `403 not_admin`, as `task.create` refuses one `403 forbidden`. Its one task, `KON-7`, is in its one module
//! and runs the team's pipeline, whose Builder profile runs on Codex.

use std::{
  io::{Read, Write},
  net::TcpStream,
  sync::Arc,
};

use orchester_core::{id::ResourceId, resource::Version};
use parking_lot::Mutex;
use serde_json::{Value, json};

use crate::{
  fixture::{self, Config},
  http::{Request, respond},
  team_server::{Extra, MEMBER, State},
};

/// What the tools' reads and edits did on the server.
#[derive(Default)]
pub struct Seen {
  /// Every task definition edit that reached the server, refused or not.
  pub task_edits: usize,
  /// Every `task.create` that reached the server, refused or not.
  pub task_creates: usize,
}

/// The routes, and what they saw.
pub fn routes(config: &Config) -> (Extra, Arc<Mutex<Seen>>) {
  let seen = Arc::new(Mutex::new(Seen::default()));
  let versions = Arc::new(Versions::of(config));
  let counted = seen.clone();
  let extra: Extra = Arc::new(move |state, request, stream| route(state, (&versions, &counted), request, stream));
  (extra, seen)
}

/// The team's published pipeline and profile, as their versions read.
struct Versions {
  pipeline: (ResourceId, Value),
  profile: (ResourceId, Value),
}

impl Versions {
  fn of(config: &Config) -> Self {
    let profile = fixture::builder();
    let connection = profile.body().connection();
    let pipeline = fixture::one_step(config.profile, "Say hello.");
    Self {
      pipeline: (config.pipeline, version(config.pipeline, &pipeline, &json!([]))),
      profile: (
        config.profile,
        version(
          config.profile,
          &profile,
          &json!([{"kind": "connection", "id": connection, "name": "Codex", "role": "codex_cli"}]),
        ),
      ),
    }
  }
}

fn version<B: serde::Serialize>(resource: ResourceId, version: &Version<B>, references: &Value) -> Value {
  json!({
    "resource": resource, "number": version.number().get(), "body": version.body(), "hash": "a".repeat(64),
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
  let path = request.path.split('?').next().unwrap_or_default();
  if let Some(route) = path.strip_prefix(&format!("/api/v1/projects/{project}/tasks/")) {
    seen.lock().task_edits += 1;
    let body = if request.token == MEMBER {
      (
        403,
        json!({"error": "not_admin", "message": "Only an admin changes this."}),
      )
    } else {
      (404, json!({"error": "not_found", "message": route}))
    };
    respond(stream, body.0, &body.1);
    return true;
  }
  let Some(route) = path.strip_prefix(&format!("/api/v2/projects/{project}")) else {
    return false;
  };
  let segments: Vec<&str> = route.split('/').filter(|segment| !segment.is_empty()).collect();
  let task_read = || {
    json!({
      "id": task, "number": 7, "key": "KON-7", "former_key": null, "revision": 1, "title": "Say hello",
      "brief": "Append one line.", "criteria": ["README.md ends with hello"], "pipeline": pipeline.to_string(),
      "limits": null, "workflow": "todo", "priority": "medium", "tags": [], "blocking": [], "blocked_reason": null,
      "module": module, "status": "queued", "has_run": false, "created": attribution(), "updated": attribution(),
      "claim": null, "run": null, "assignee": null, "work": null, "effective_claim": null,
      "capabilities": capabilities(project)
    })
  };
  let summary = || {
    json!({
      "module": {
        "id": module, "name": "Webhooks", "description": "", "start": null, "target": null, "state": "open",
        "revision": 1, "created": attribution(), "updated": attribution(), "branch": "module/webhooks",
        "assignee": null, "work": null, "effective_claim": null, "capabilities": capabilities(project)
      },
      "progress": {"unit": "tasks", "scope": 1, "done": 0}, "tasks": 1,
      "forecast": {"remaining": 1, "history_days": 0, "history_delivered": 0, "outlook": {"kind": "not_enough_history"}}
    })
  };
  let page = |items: Vec<Value>| json!({"items": items, "next": null, "cursor": "10"});
  let (status, body) = match (request.method.as_str(), segments.as_slice()) {
    ("GET", ["subscription"]) => return subscription(stream),
    (_, ["task-list"]) => {
      let search = request.json()["filters"]["search"].as_str().map(str::to_lowercase);
      let found = search.is_none_or(|search| "kon-7 say hello".contains(&search));
      (200, page(if found { vec![task_read()] } else { Vec::new() }))
    }
    ("GET", ["tasks", id]) if *id == task.to_string() => (200, task_read()),
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
      seen.lock().task_creates += 1;
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
