//! The stand-in's task list as the `/api/v2` reads give it: its tasks as `TaskRead`s, the request's filters and search,
//! its sort, pages over an opaque cursor that names the snapshot and the offset, the board counts, and the project's
//! module with its progress.

use orchester_core::time::Timestamp;
use serde_json::{Value, json};

use super::{AuthorityTask, State, capabilities};
use crate::{
  test_support::team_server::{
    TasksAnswer, USER,
    first_download::{self, PAGED, TASKS},
  },
  v2::{TaskFilters, TaskListRequest},
};

/// The workflow statuses and priorities as the wire names them, in the list's orders.
const WORKFLOW: [&str; 6] = ["backlog", "todo", "in_progress", "in_review", "done", "canceled"];
const PRIORITY: [&str; 5] = ["urgent", "high", "medium", "low", "none"];

/// The id of the task numbered `number` that [`super::TeamServer::authority_tasks`] set.
pub(super) fn authority_id(number: u32) -> String {
  format!("0191c7c0-0000-7000-8000-0000001{number:05x}")
}

/// The project's tasks as `TaskRead`s: those [`super::TeamServer::authority_tasks`] set, or else the v1 tasks.
pub(super) fn tasks((project, key): (&str, &str), state: &State) -> Vec<Value> {
  let standing = state.authority.standing;
  let read = |task: Value| -> Value {
    let mut task = task;
    task["assignee"] = Value::Null;
    task["work"] = Value::Null;
    task["effective_claim"] = Value::Null;
    task["capabilities"] = capabilities(project, standing);
    task
  };
  if let Some(tasks) = &state.authority.tasks {
    return tasks.iter().map(|task| read(authority_task(task, key))).collect();
  }
  match (state.tasks, state.pages) {
    (TasksAnswer::Empty, _) => Vec::new(),
    (_, Some(pages)) => (1..=pages.total)
      .map(|number| {
        let (title, workflow) = PAGED[number % PAGED.len()];
        let id = format!("0191c7c0-0000-7000-8000-0000000{number:05x}");
        read(first_download::task(&id, (key, number), (title, workflow)))
      })
      .collect(),
    (_, None) => TASKS
      .iter()
      .enumerate()
      .map(|(index, (id, title))| read(first_download::task(id, (key, index + 1), (title, "todo"))))
      .collect(),
  }
}

fn authority_task(task: &AuthorityTask, key: &str) -> Value {
  let number = usize::try_from(task.number).unwrap_or(usize::MAX);
  let mut read = first_download::task(&authority_id(task.number), (key, number), (&task.title, task.workflow));
  let created = Timestamp::now().unix_millis() - task.created_minutes_ago * 60_000;
  read["priority"] = json!(task.priority);
  read["pipeline"] = json!(task.pipeline);
  read["brief"] = json!(task.title);
  read["criteria"] = json!([task.title]);
  read["tags"] = json!([]);
  read["module"] = if task.module {
    json!(first_download::MODULE)
  } else {
    Value::Null
  };
  read["created"] = json!({"by": {"id": USER, "display_name": "Bohdan Koval"}, "at": created});
  read["updated"] = read["created"].clone();
  read
}

/// The tasks `filters` keep.
fn kept<'a>(tasks: &'a [Value], filters: &TaskFilters) -> Vec<&'a Value> {
  let wire = serde_json::to_value(filters).unwrap_or(Value::Null);
  let any = |facet: &str, value: &Value| {
    wire[facet]
      .as_array()
      .is_none_or(|values| values.is_empty() || values.contains(value))
  };
  let search = filters.search.as_ref().map(|search| search.as_str().to_lowercase());
  tasks
    .iter()
    .filter(|task| any("statuses", &task["workflow"]) && any("priorities", &task["priority"]))
    .filter(|task| any("modules", &task["module"]))
    .filter(|task| {
      search.as_ref().is_none_or(|search| {
        ["title", "key"]
          .iter()
          .any(|field| task[field].as_str().unwrap_or_default().to_lowercase().contains(search))
      })
    })
    .collect()
}

/// Where `value` is in `order`, for sorting by it.
fn position(order: &[&str], value: &Value) -> usize {
  order
    .iter()
    .position(|named| value.as_str() == Some(named))
    .unwrap_or(order.len())
}

/// `POST …/task-list`: one page of the kept tasks in the request's order. A cursor is `<snapshot>:<offset>`.
pub(super) fn page(tasks: &[Value], body: &str, cursor: u64) -> Result<Value, (u16, &'static str)> {
  let request: TaskListRequest = serde_json::from_str(body).map_err(|_| (422, "invalid"))?;
  let offset = match request.page.after() {
    None => 0,
    Some(after) => after
      .as_str()
      .split_once(':')
      .and_then(|(_, offset)| offset.parse().ok())
      .ok_or((422, "invalid"))?,
  };
  let mut kept = kept(tasks, &request.filters);
  let number = |task: &Value| task["number"].as_u64().unwrap_or_default();
  match serde_json::to_value(request.sort).unwrap_or(Value::Null).as_str() {
    Some("newest") => kept.sort_by_key(|task| std::cmp::Reverse(number(task))),
    Some("title") => kept.sort_by_key(|task| task["title"].as_str().unwrap_or_default().to_lowercase()),
    Some("status") => kept.sort_by_key(|task| position(&WORKFLOW, &task["workflow"])),
    Some("priority") => kept.sort_by_key(|task| position(&PRIORITY, &task["priority"])),
    _ => kept.sort_by_key(|task| number(task)),
  }
  let end = (offset + usize::from(request.page.limit())).min(kept.len());
  let items: Vec<&Value> = kept.get(offset..end).unwrap_or_default().to_vec();
  let next = (end < kept.len()).then(|| format!("{cursor}:{end}"));
  Ok(json!({"items": items, "next": next, "cursor": cursor.to_string()}))
}

/// `POST …/task-counts`: the kept tasks by workflow status.
pub(super) fn counts(tasks: &[Value], body: &str, cursor: u64) -> Result<Value, (u16, &'static str)> {
  let filters: TaskFilters = serde_json::from_str(body).map_err(|_| (422, "invalid"))?;
  let mut counts = json!({"backlog": 0, "todo": 0, "in_progress": 0, "in_review": 0, "done": 0, "canceled": 0});
  for task in kept(tasks, &filters) {
    let status = task["workflow"].as_str().unwrap_or("backlog");
    counts[status] = json!(counts[status].as_u64().unwrap_or_default() + 1);
  }
  counts["cursor"] = json!(cursor.to_string());
  Ok(counts)
}

/// `POST …/module-list`: the project's one module, unless the project is empty.
pub(super) fn modules(project: &str, state: &State) -> Value {
  let items: Vec<Value> = summary(project, state).into_iter().collect();
  json!({"items": items, "next": null, "cursor": state.authority.cursor().to_string()})
}

/// `GET …/modules/{module}` and `GET …/modules/{module}/progress`, `path` being what follows the project; `None` for no
/// module of the project.
pub(super) fn module(project: &str, path: &str, state: &State) -> Option<Value> {
  let rest = path.strip_prefix("modules/")?;
  let (id, part) = rest.split_once('/').unwrap_or((rest, ""));
  let summary = summary(project, state).filter(|_| id == first_download::MODULE)?;
  match part {
    "" => Some(summary),
    "progress" => Some(summary["progress"].clone()),
    _ => None,
  }
}

/// The project's one module as a `ModuleSummary`, with its progress over its tasks as core counts it; `None` while the
/// project is empty.
fn summary(project: &str, state: &State) -> Option<Value> {
  if state.authority.tasks.is_none() && state.tasks == TasksAnswer::Empty {
    return None;
  }
  let key = state
    .projects
    .iter()
    .find(|shared| shared["id"] == project)
    .and_then(|shared| shared["key"].as_str())
    .unwrap_or_default();
  let tasks = tasks((project, key), state);
  let inside: Vec<&Value> = tasks
    .iter()
    .filter(|task| task["module"] == first_download::MODULE)
    .collect();
  let scope = inside.iter().filter(|task| task["workflow"] != "canceled").count();
  let done = inside.iter().filter(|task| task["workflow"] == "done").count();
  let mut module = first_download::module();
  module["branch"] = json!("module/payments");
  module["assignee"] = Value::Null;
  module["work"] = Value::Null;
  module["effective_claim"] = Value::Null;
  module["capabilities"] = capabilities(project, state.authority.standing);
  Some(json!({
    "module": module, "progress": {"unit": "tasks", "scope": scope, "done": done}, "tasks": inside.len(),
    "forecast": {"remaining": scope - done, "history_days": 0, "history_delivered": 0,
      "outlook": {"kind": "not_enough_history"}},
  }))
}
