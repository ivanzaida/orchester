//! What a checkout linked to a project on the stand-in team server reads in a round of the shared projects' work: the
//! project, its one tag, its one module and its three tasks, or as many as [`TaskPages`] says, page by page, or nothing
//! of them while the project is empty. Its tasks can be served, refused with a `500`, or held, for a QA copy to show
//! the first download under way. A first link of a project sends its tasks (`POST /projects/{id}/link`), which the
//! server can refuse.

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
/// The most tasks a page holds, as a real server pages them (`docs/architecture/server.md`).
const PAGE: usize = 64;

const TAG: &str = "0191c7c0-0000-7000-8000-000000000a01";
const MODULE: &str = "0191c7c0-0000-7000-8000-000000000a02";
const TASKS: [(&str, &str); 3] = [
  ("0191c7c0-0000-7000-8000-000000000e01", "Sign in with a passkey"),
  ("0191c7c0-0000-7000-8000-000000000e02", "Export the ledger"),
  ("0191c7c0-0000-7000-8000-000000000e03", "Billing retries"),
];
/// The titles and workflow statuses of paged tasks, by number: frame 08's sample (`YN6MO`).
const PAGED: [(&str, &str); 3] = [
  ("Retry webhook delivery on 5xx", "todo"),
  ("Export invoices as PDF", "in_review"),
  ("Show the signed-in account in the menu", "in_progress"),
];

/// How the server answers a project's tasks.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TasksAnswer {
  /// No task, tag or module: a first link of a checkout uploads its own.
  #[default]
  Empty,
  Serve,
  /// Held for at most [`HOLD_AT_MOST`], until another answer is set; then refused like [`Self::Fail`].
  Hold,
  /// `500 internal`.
  Fail,
}

/// A project's tasks in pages: `total` in all, those up to number `until` served whatever [`TasksAnswer`] says, and
/// the rest as it says.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct TaskPages {
  pub(super) total: usize,
  pub(super) until: usize,
}

/// The answer to `GET /projects/{id}` and what is under it, and to a first link's `POST`, for the `path` of one of
/// `state`'s projects; `None` for anything else.
pub(super) fn answer((path, query): (&str, &str), state: &Mutex<State>) -> Option<(&'static str, Value)> {
  let (verb, rest) = path.split_once(" /api/v1/projects/")?;
  let (id, part) = rest.split_once('/').unwrap_or((rest, ""));
  let project = state
    .lock()
    .projects
    .iter()
    .find(|project| project["id"] == id)
    .cloned()?;
  let empty = state.lock().tasks == TasksAnswer::Empty;
  match (verb, part) {
    ("GET", "") => Some(("200 OK", json!({"project": project}))),
    ("GET", "tags") if empty => Some(("200 OK", json!({"tags": []}))),
    ("GET", "tags") => Some(("200 OK", json!({"tags": [{"id": TAG, "name": "backend"}]}))),
    ("GET", "modules") if empty => Some(("200 OK", json!({"modules": [], "changes": []}))),
    ("GET", "modules") => Some(("200 OK", json!({"modules": [module()], "changes": []}))),
    ("GET", "tasks") => Some(tasks(&project, query, state)),
    ("POST", "link") if state.lock().refuses_links => Some(("403 Forbidden", error("forbidden"))),
    ("POST", "link") => Some(("200 OK", json!({"tasks": [], "dropped_links": [], "cursor": 0}))),
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

fn tasks(project: &Value, query: &str, state: &Mutex<State>) -> (&'static str, Value) {
  state.lock().tasks_asked += 1;
  let key = project["key"].as_str().expect("`share` gives each project its key");
  let after = query
    .strip_prefix("after=")
    .and_then(|after| after.parse().ok())
    .unwrap_or(0);
  let pages = state.lock().pages;
  if let Some(pages) = pages.filter(|pages| after < pages.until) {
    return ("200 OK", page(key, after, pages.until, pages.total));
  }
  let deadline = Instant::now() + HOLD_AT_MOST;
  while state.lock().tasks == TasksAnswer::Hold && Instant::now() < deadline {
    thread::sleep(HOLD_POLL);
  }
  let answer = state.lock().tasks;
  match (answer, pages) {
    (TasksAnswer::Empty, _) => ("200 OK", json!({"tasks": [], "cursor": 0})),
    (TasksAnswer::Serve, Some(pages)) => ("200 OK", page(key, after, pages.total, pages.total)),
    (TasksAnswer::Serve, None) => {
      let tasks: Vec<Value> = TASKS
        .iter()
        .enumerate()
        .map(|(index, (id, title))| task(id, (key, index + 1), (title, "todo")))
        .collect();
      ("200 OK", json!({"tasks": tasks, "cursor": 7}))
    }
    (TasksAnswer::Hold | TasksAnswer::Fail, _) => ("500 Internal Server Error", error("internal")),
  }
}

/// The page of the tasks numbered above `after`, at most [`PAGE`] of them and none above `until`, of `total` in all.
fn page(key: &str, after: usize, until: usize, total: usize) -> Value {
  let last = (after + PAGE).min(until);
  let tasks: Vec<Value> = (after + 1..=last)
    .map(|number| {
      let (title, workflow) = PAGED[number % PAGED.len()];
      let id = format!("0191c7c0-0000-7000-8000-0000000{number:05x}");
      task(&id, (key, number), (title, workflow))
    })
    .collect();
  let mut page = json!({"tasks": tasks, "cursor": 7, "total": total});
  if last < total {
    page["next"] = json!(last);
  }
  page
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

fn task(id: &str, (key, number): (&str, usize), (title, workflow): (&str, &str)) -> Value {
  json!({
    "id": id, "number": number, "key": format!("{key}-{number}"), "former_key": null, "revision": 1,
    "title": title, "brief": "", "criteria": [], "pipeline": null, "limits": null, "workflow": workflow,
    "priority": "high", "tags": [TAG], "blocking": [], "blocked_reason": null, "module": MODULE,
    "status": "draft", "has_run": false, "created": attribution(), "updated": attribution(), "claim": null,
    "run": null,
  })
}
