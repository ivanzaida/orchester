//! What a checkout linked to a project on the stand-in team server reads in a round of the shared projects' work: the
//! project, its one tag, its one module and its three tasks. Its tasks can be served, refused with a `500`, or held,
//! for a QA copy to show the first download under way.

use std::{
  thread,
  time::{Duration, Instant},
};

use parking_lot::Mutex;
use serde_json::{Value, json};

use super::{State, USER, error};

/// The longest a held answer waits before it is refused. The client gives up on a request after 30 seconds and then
/// takes the server for unreachable, so a held answer ends before that and leaves the server connected.
const HOLD_AT_MOST: Duration = Duration::from_secs(25);
/// How often a held answer looks whether it may end.
const HOLD_POLL: Duration = Duration::from_millis(20);

const TAG: &str = "0191c7c0-0000-7000-8000-000000000a01";
const MODULE: &str = "0191c7c0-0000-7000-8000-000000000a02";
const TASKS: [(&str, &str); 3] = [
  ("0191c7c0-0000-7000-8000-000000000e01", "Sign in with a passkey"),
  ("0191c7c0-0000-7000-8000-000000000e02", "Export the ledger"),
  ("0191c7c0-0000-7000-8000-000000000e03", "Billing retries"),
];

/// How the server answers a project's tasks.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TasksAnswer {
  #[default]
  Serve,
  /// Held for at most [`HOLD_AT_MOST`], until another answer is set; then refused like [`Self::Fail`].
  Hold,
  /// `500 internal`.
  Fail,
}

/// The answer to `GET /projects/{id}` and what is under it, for the `path` of one of `state`'s projects; `None` for
/// anything else.
pub(super) fn answer(path: &str, state: &Mutex<State>) -> Option<(&'static str, Value)> {
  let rest = path.strip_prefix("GET /api/v1/projects/")?;
  let (id, part) = rest.split_once('/').unwrap_or((rest, ""));
  let project = state
    .lock()
    .projects
    .iter()
    .find(|project| project["id"] == id)
    .cloned()?;
  match part {
    "" => Some(("200 OK", json!({"project": project}))),
    "tags" => Some(("200 OK", json!({"tags": [{"id": TAG, "name": "backend"}]}))),
    "modules" => Some(("200 OK", json!({"modules": [module()], "changes": []}))),
    "tasks" => Some(tasks(&project, state)),
    _ => None,
  }
}

/// The answer to `GET /projects?slug=`: `404` when the person is a member of no such project.
pub(super) fn by_slug(slug: &str, state: &Mutex<State>) -> (&'static str, Value) {
  let found = state
    .lock()
    .projects
    .iter()
    .find(|project| project["slug"] == slug)
    .cloned();
  match found {
    Some(project) => ("200 OK", json!({"project": project})),
    None => ("404 Not Found", error("not_found")),
  }
}

fn tasks(project: &Value, state: &Mutex<State>) -> (&'static str, Value) {
  state.lock().tasks_asked += 1;
  let deadline = Instant::now() + HOLD_AT_MOST;
  while state.lock().tasks == TasksAnswer::Hold && Instant::now() < deadline {
    thread::sleep(HOLD_POLL);
  }
  let answer = state.lock().tasks;
  match answer {
    TasksAnswer::Serve => {
      let key = project["key"].as_str().expect("`share` gives each project its key");
      let tasks: Vec<Value> = TASKS
        .iter()
        .enumerate()
        .map(|(index, (id, title))| task(id, (key, index + 1), title))
        .collect();
      ("200 OK", json!({"tasks": tasks, "cursor": 7}))
    }
    TasksAnswer::Hold | TasksAnswer::Fail => ("500 Internal Server Error", error("internal")),
  }
}

fn attribution() -> Value {
  json!({"by": {"id": USER, "display_name": "Bohdan Koval"}, "at": 1_000})
}

fn module() -> Value {
  json!({
    "id": MODULE, "name": "Payments", "description": "", "start": null, "target": null, "state": "open",
    "revision": 1, "created": attribution(), "updated": attribution(),
  })
}

fn task(id: &str, (key, number): (&str, usize), title: &str) -> Value {
  json!({
    "id": id, "number": number, "key": format!("{key}-{number}"), "former_key": null, "revision": 1,
    "title": title, "brief": "", "criteria": [], "pipeline": null, "limits": null, "workflow": "todo",
    "priority": "high", "tags": [TAG], "blocking": [], "blocked_reason": null, "module": MODULE,
    "status": "draft", "has_run": false, "created": attribution(), "updated": attribution(), "claim": null,
    "run": null,
  })
}
