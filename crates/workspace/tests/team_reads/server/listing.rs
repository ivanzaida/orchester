//! The fake's task list: the request's filters and search, its sort, pages over an opaque cursor that names the
//! snapshot and the offset, the board counts, and the modules' summaries and progress.

use std::net::TcpStream;

use orchester_api::v2::{ModuleListRequest, TaskFilters, TaskListRequest};
use orchester_core::id::{ModuleId, ProjectId};
use serde_json::{Value, json};

use super::{Shared, State, Task};
use crate::http::{Request, failure, refusal, respond};

const USER: &str = "01a183cf-b73c-71db-8a62-bd509770cd4e";

pub fn capabilities(project: ProjectId) -> Value {
  json!({"project_id": project, "revision": 1, "role": "member",
    "actions": [{"action": "read", "subject": null, "allowed": true, "reason": null}], "unresolved": []})
}

fn attribution() -> Value {
  json!({ "by": { "id": USER, "display_name": "Alice" }, "at": 1 })
}

/// `task` as a `TaskRead`.
pub fn task_read(project: ProjectId, task: &Task) -> Value {
  json!({
    "id": task.id, "number": task.number, "key": format!("KON-{}", task.number), "former_key": null,
    "revision": task.revision, "title": task.title, "brief": "", "criteria": [], "pipeline": null, "limits": null,
    "workflow": task.workflow, "priority": task.priority, "tags": [], "blocking": [], "blocked_reason": null,
    "module": task.module, "status": "draft", "has_run": false, "created": attribution(), "updated": attribution(),
    "claim": null, "run": null, "assignee": null, "work": null, "effective_claim": null,
    "capabilities": capabilities(project)
  })
}

/// The tasks `filters` keep, as the request's JSON names statuses, priorities and modules.
fn kept<'a>(state: &'a State, filters: &TaskFilters) -> Vec<&'a Task> {
  let wire = serde_json::to_value(filters).expect("filters serialize");
  let any = |facet: &str, value: Value| {
    let values = wire[facet].as_array().expect("a facet");
    values.is_empty() || values.contains(&value)
  };
  state
    .tasks
    .iter()
    .filter(|task| any("statuses", json!(task.workflow)) && any("priorities", json!(task.priority)))
    .filter(|task| any("modules", json!(task.module)))
    .filter(|task| {
      filters
        .search
        .as_ref()
        .is_none_or(|search| task.title.to_lowercase().contains(&search.as_str().to_lowercase()))
    })
    .collect()
}

/// `POST …/task-list`: one page of the kept tasks in the request's order. A cursor is `<snapshot>:<offset>`; one whose
/// snapshot is gone is `410 cursor_expired`.
pub fn page(shared: &Shared, project: ProjectId, request: &Request, stream: &mut TcpStream) {
  let request: TaskListRequest = request.json();
  let state = shared.state.lock().expect("state");
  let (snapshot, offset) = match request.page.after() {
    None => (state.cursor(), 0),
    Some(after) => {
      let (snapshot, offset) = after.as_str().split_once(':').expect("the fake's cursor");
      (
        snapshot.parse().expect("a snapshot"),
        offset.parse().expect("an offset"),
      )
    }
  };
  if snapshot <= state.expired_through {
    drop(state);
    return refusal(stream, 410, "cursor_expired");
  }
  let mut tasks = kept(&state, &request.filters);
  if serde_json::to_value(request.sort).expect("a sort") == json!("title") {
    tasks.sort_by_key(|task| task.title.to_lowercase());
  }
  let limit = usize::from(request.page.limit());
  let end = (offset + limit).min(tasks.len());
  let items: Vec<Value> = tasks[offset..end].iter().map(|task| task_read(project, task)).collect();
  let next = (end < tasks.len()).then(|| format!("{snapshot}:{end}"));
  let body = json!({ "items": items, "next": next, "cursor": snapshot.to_string() });
  drop(state);
  respond(stream, 200, &body);
}

/// `POST …/task-counts`: the kept tasks by workflow status.
pub fn counts(shared: &Shared, request: &Request, stream: &mut TcpStream) {
  let filters: TaskFilters = request.json();
  let state = shared.state.lock().expect("state");
  let mut counts = json!({"backlog": 0, "todo": 0, "in_progress": 0, "in_review": 0, "done": 0, "canceled": 0});
  for task in kept(&state, &filters) {
    counts[task.workflow] = json!(counts[task.workflow].as_u64().expect("a count") + 1);
  }
  counts["cursor"] = json!(state.cursor().to_string());
  drop(state);
  respond(stream, 200, &counts);
}

/// The module's tasks, how many of them are in its scope (not Canceled) and how many are Done, as core counts them.
fn counted(state: &State) -> (usize, usize, usize) {
  let tasks: Vec<&Task> = state
    .tasks
    .iter()
    .filter(|task| task.module == Some(state.module))
    .collect();
  let scope = tasks.iter().filter(|task| task.workflow != "canceled").count();
  let done = tasks.iter().filter(|task| task.workflow == "done").count();
  (tasks.len(), scope, done)
}

/// `POST …/module-list`: the project's one module with its progress over its tasks.
pub fn modules(shared: &Shared, project: ProjectId, request: &Request, stream: &mut TcpStream) {
  // The fake has one module, open, so it reads the request only to check it is the contract's.
  let _: ModuleListRequest = request.json();
  let state = shared.state.lock().expect("state");
  let (tasks, scope, done) = counted(&state);
  let module = json!({
    "id": state.module, "name": "Sign-in", "description": "", "start": null, "target": null, "state": "open",
    "revision": 1, "created": attribution(), "updated": attribution(), "branch": "module/sign-in",
    "assignee": null, "work": null, "effective_claim": null, "capabilities": capabilities(project),
    "pending_delivery": null
  });
  let summary = json!({
    "module": module, "progress": {"unit": "tasks", "scope": scope, "done": done}, "tasks": tasks,
    "forecast": {"remaining": scope - done, "history_days": 0, "history_delivered": 0,
      "outlook": {"kind": "not_enough_history"}}
  });
  let body = json!({ "items": [summary], "next": null, "cursor": state.cursor().to_string() });
  drop(state);
  respond(stream, 200, &body);
}

/// `GET …/modules/{module}/progress`: `ModuleProgressRead`, with no cursor of its own; `404 not_found` for another
/// module.
pub fn progress(shared: &Shared, module: ModuleId, stream: &mut TcpStream) {
  let state = shared.state.lock().expect("state");
  if module != state.module {
    drop(state);
    return failure(stream, 404, "not_found");
  }
  let (_, scope, done) = counted(&state);
  drop(state);
  respond(stream, 200, &json!({"unit": "tasks", "scope": scope, "done": done}));
}
