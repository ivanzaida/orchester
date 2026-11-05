//! The stand-in's task list as the `/api/v2` reads give it: its tasks as `TaskRead`s, the request's filters and search,
//! its sort, pages over an opaque cursor that names the snapshot and the offset, the board counts, and the project's
//! modules with their progress: the ones [`super::TeamServer::authority_modules`] set, or else its one module.

use orchester_core::time::Timestamp;
use serde_json::{Value, json};

use super::{AuthorityClaim, AuthorityTask, State, capabilities, commands, edits, work};
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

/// The id of a task that [`super::TeamServer::authority_tasks`] set: its own, or one made from its number.
pub(super) fn task_id(task: &AuthorityTask) -> String {
  task
    .id
    .clone()
    .unwrap_or_else(|| format!("0191c7c0-0000-7000-8000-0000001{:05x}", task.number))
}

/// The project's tasks as `TaskRead`s: those [`super::TeamServer::authority_tasks`] set, or else the v1 tasks, with
/// what Admins changed of them here.
pub(super) fn tasks(project: (&str, &str), state: &State) -> Vec<Value> {
  listed(project, state)
    .into_iter()
    .map(|task| edits::applied(&state.authority, task))
    .collect()
}

fn listed((project, key): (&str, &str), state: &State) -> Vec<Value> {
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
    return tasks
      .iter()
      .map(|task| authority_task(task, (project, key), state))
      .collect();
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

/// `task` of `project` as a `TaskRead`, with the caller's capabilities on it. Its claim is its own, else its module's.
fn authority_task(task: &AuthorityTask, (project, key): (&str, &str), state: &State) -> Value {
  let number = usize::try_from(task.number).unwrap_or(usize::MAX);
  let id = task_id(task);
  let mut read = first_download::task(&id, (key, number), (&task.title, task.workflow));
  let created = Timestamp::now().unix_millis() - task.created_minutes_ago * 60_000;
  read["priority"] = json!(task.priority);
  read["pipeline"] = json!(task.pipeline);
  read["brief"] = json!(task.title);
  read["criteria"] = json!([task.title]);
  read["tags"] = json!([]);
  let modules = state.authority.modules.as_deref();
  read["module"] = match (task.module, modules) {
    (Some(index), Some(_)) => json!(work::module_id(index)),
    (Some(_), None) => json!(first_download::MODULE),
    (None, _) => Value::Null,
  };
  read["created"] = json!({"by": {"id": USER, "display_name": "Bohdan Koval"}, "at": created});
  read["updated"] = read["created"].clone();
  read["assignee"] = task.assignee.map_or(Value::Null, work::Person::wire);
  read["work"] = match (task.claim, task.work) {
    (Some(claim), Some(state)) => work_read((&id, task.number), claim, state),
    _ => Value::Null,
  };
  let module_claim = task
    .module
    .and_then(|index| modules?.get(index)?.claim.map(|claim| (claim, index)));
  read["effective_claim"] = match (task.claim, module_claim) {
    (Some(claim), _) => claim.wire(&json!({"kind": "task", "id": id}), task.number),
    (None, Some((claim, index))) => {
      let subject = json!({"kind": "module", "id": work::module_id(index)});
      claim.wire(&subject, work::module_number(index))
    }
    (None, None) => Value::Null,
  };
  let module_held = module_claim.map(|(claim, _)| claim.holder);
  read["capabilities"] =
    commands::task_capabilities(&state.authority, (project, &id), (task.claim.as_ref(), module_held));
  read
}

/// The work of task `id` numbered `number` under `claim`, in `state` (`running`, `awaiting_human`, …).
pub(super) fn work_read((id, number): (&str, u32), claim: AuthorityClaim, state: &str) -> Value {
  let at = Timestamp::now().unix_millis();
  json!({"id": format!("0191c7c0-0000-7000-8000-0000004{number:05x}"), "subject": {"kind": "task", "id": id},
    "claim_id": claim.id(number), "revision": 1, "state": state, "runs": [],
    "bundle": {"sha256": "0".repeat(64), "schema_version": 2}, "created_at": at, "updated_at": at})
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
    .filter(|task| any("modules", &task["module"]) && any("releases", &task["release"]))
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

/// `POST …/module-list`: the project's modules, none while the project is empty.
pub(super) fn modules(project: (&str, &str), state: &State) -> Value {
  json!({"items": summaries(project, state), "next": null, "cursor": state.authority.cursor().to_string()})
}

/// `GET …/modules/{module}` and `GET …/modules/{module}/progress`, `path` being what follows the project; `None` for no
/// module of the project.
pub(super) fn module(project: (&str, &str), path: &str, state: &State) -> Option<Value> {
  let rest = path.strip_prefix("modules/")?;
  let (id, part) = rest.split_once('/').unwrap_or((rest, ""));
  let summary = summaries(project, state)
    .into_iter()
    .find(|summary| summary["module"]["id"] == id)?;
  match part {
    "" => Some(summary),
    "progress" => Some(summary["progress"].clone()),
    _ => None,
  }
}

/// The project's modules as `ModuleSummary`s, each with its progress over its tasks as core counts it: those
/// [`super::TeamServer::authority_modules`] set, or else its one module; none while the project is empty.
fn summaries((project, key): (&str, &str), state: &State) -> Vec<Value> {
  if state.authority.tasks.is_none() && state.tasks == TasksAnswer::Empty {
    return Vec::new();
  }
  let tasks = tasks((project, key), state);
  let capabilities = || capabilities(project, state.authority.standing);
  let Some(modules) = &state.authority.modules else {
    let mut module = first_download::module();
    module["branch"] = json!("module/payments");
    module["assignee"] = Value::Null;
    module["work"] = Value::Null;
    module["effective_claim"] = Value::Null;
    module["capabilities"] = capabilities();
    module["pending_delivery"] = Value::Null;
    let (progress, count, remaining) = progress(&tasks, first_download::MODULE);
    let forecast = json!({"remaining": remaining, "history_days": 0, "history_delivered": 0,
      "outlook": {"kind": "not_enough_history"}});
    return vec![json!({"module": module, "progress": progress, "tasks": count, "forecast": forecast})];
  };
  modules
    .iter()
    .enumerate()
    .map(|(index, module)| {
      let (progress, count, remaining) = progress(&tasks, &work::module_id(index));
      json!({"module": commands::module_read(&state.authority, project, index), "progress": progress,
        "tasks": count, "forecast": module.forecast(remaining)})
    })
    .collect()
}

/// The progress of the module `id` over `tasks`, how many tasks it has and how many remain.
fn progress(tasks: &[Value], id: &str) -> (Value, usize, usize) {
  let inside: Vec<&Value> = tasks.iter().filter(|task| task["module"] == id).collect();
  let scope = inside.iter().filter(|task| task["workflow"] != "canceled").count();
  let done = inside.iter().filter(|task| task["workflow"] == "done").count();
  (
    json!({"unit": "tasks", "scope": scope, "done": done}),
    inside.len(),
    scope - done,
  )
}
