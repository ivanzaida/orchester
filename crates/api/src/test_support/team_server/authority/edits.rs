//! A task's definition as an Admin changes it on the stand-in (`PATCH /api/v1/projects/{p}/tasks/{t}`,
//! `docs/cloud-api.md` "Team authority P2 commands"): on the revision read, a member's change `403 not_admin`, a stale
//! revision `409 revision_conflict` with the task as it is now. What changed is laid over the task's reads from then
//! on, at its new revision and in the name of who changed it; someone else can change a task first, just before the
//! next change asked here, as when another Admin saved it meanwhile.

use orchester_core::time::Timestamp;
use parking_lot::Mutex;
use serde_json::{Map, Value, json};

use super::{Authority, Person, Standing, listing};
use crate::{TaskPatch, test_support::team_server::State};

/// `PATCH /api/v1/projects/{p}/tasks/{t}` as `path` names it (method first), with `body`: the status line and the
/// answer; `None` for another request.
pub(in crate::test_support::team_server) fn answer(
  path: &str,
  body: &str,
  state: &Mutex<State>,
) -> Option<(&'static str, Value)> {
  let rest = path.strip_prefix("PATCH /api/v1/projects/")?;
  let (project, task) = rest.split_once("/tasks/")?;
  let mut state = state.lock();
  let key = state
    .projects
    .iter()
    .find(|known| known["id"] == project)
    .and_then(|known| known["key"].as_str())
    .map(str::to_owned);
  let Some(key) = key else {
    return Some(("404 Not Found", json!({"error": "not_found", "message": "refused"})));
  };
  let (status, answer) = patch(&mut state, ((project, &key), task), body);
  let line = match status {
    200 => "200 OK",
    403 => "403 Forbidden",
    404 => "404 Not Found",
    409 => "409 Conflict",
    _ => "422 Unprocessable Entity",
  };
  Some((line, answer))
}

/// The fields of a task's content a change sets when it names them: absent or `null` leaves them.
const SET: [&str; 6] = ["title", "brief", "criteria", "priority", "tags", "blocking"];
/// The fields a change may set to `null`: present, they take what it gives.
const NULLABLE: [&str; 2] = ["module", "blocked_reason"];

/// A task's definition as changed here: its revision, the fields as changed, who changed it last and when.
#[derive(Clone, Debug)]
pub(super) struct Edited {
  revision: u64,
  fields: Map<String, Value>,
  by: Person,
  at: i64,
}

/// Someone else's change of the task numbered `number`, made just before the next change asked here.
#[derive(Clone, Debug)]
pub(super) struct FirstChange {
  number: u32,
  by: Person,
  fields: Map<String, Value>,
}

impl super::TeamServer {
  /// `by` sets the priority of the task numbered `number` to `priority` (as the wire names it) just before the next
  /// change of a task asked here, which then finds the task at a newer revision.
  pub fn task_changed_first(&self, number: u32, by: Person, priority: &'static str) {
    let mut fields = Map::new();
    fields.insert("priority".to_owned(), json!(priority));
    self.state.lock().authority.edit_first = Some(FirstChange { number, by, fields });
  }

  /// The task numbered `number` as the reads give it now: its revision, priority and title.
  pub fn task_now(&self, number: u32) -> Option<(u64, String, String)> {
    let state = self.state.lock();
    let project = state.projects.first()?;
    let (id, key) = (project["id"].as_str()?, project["key"].as_str()?);
    let task = listing::tasks((id, key), &state)
      .into_iter()
      .find(|task| task["number"] == number)?;
    let text = |field: &str| task[field].as_str().unwrap_or_default().to_owned();
    Some((task["revision"].as_u64()?, text("priority"), text("title")))
  }
}

/// `task` with what was changed of it here laid over it.
pub(super) fn applied(authority: &Authority, mut task: Value) -> Value {
  let Some(edited) = task["id"].as_str().and_then(|id| authority.edits.get(id)) else {
    return task;
  };
  for (field, value) in &edited.fields {
    task[field] = value.clone();
  }
  task["revision"] = json!(edited.revision);
  task["updated"] = json!({"by": edited.by.wire(), "at": edited.at});
  task
}

/// `PATCH …/tasks/{task}` of the project `(id, key)` with `body`: the task as changed, or the refusal.
fn patch(state: &mut State, (project, task): ((&str, &str), &str), body: &str) -> (u16, Value) {
  if state.authority.standing == Standing::Member {
    return (403, json!({"error": "not_admin", "message": "refused"}));
  }
  let Ok(patch) = serde_json::from_str::<TaskPatch>(body) else {
    return (422, json!({"error": "invalid", "message": "refused"}));
  };
  if let Some(first) = state.authority.edit_first.take() {
    let number = |read: &Value| read["number"] == first.number;
    if let Some(read) = listing::tasks(project, state).into_iter().find(number) {
      change(&mut state.authority, &read, (first.fields, first.by));
    }
  }
  let Some(current) = listing::tasks(project, state)
    .into_iter()
    .find(|read| read["id"] == task)
  else {
    return (404, json!({"error": "not_found", "message": "refused"}));
  };
  if current["revision"].as_u64() != Some(patch.revision.get()) {
    return (
      409,
      json!({"error": "revision_conflict", "message": "refused", "current": current}),
    );
  }
  let asked = serde_json::to_value(&patch.changes).unwrap_or(Value::Null);
  let mut fields = Map::new();
  for field in SET {
    if !asked[field].is_null() {
      fields.insert(field.to_owned(), asked[field].clone());
    }
  }
  for field in NULLABLE {
    if let Some(value) = asked.get(field) {
      fields.insert(field.to_owned(), value.clone());
    }
  }
  change(&mut state.authority, &current, (fields, Person::You));
  let changed = listing::tasks(project, state)
    .into_iter()
    .find(|read| read["id"] == task);
  (200, json!({"task": changed}))
}

/// Lays `fields` over the task `read`, at its next revision in `by`'s name, committed as one batch.
fn change(authority: &mut Authority, read: &Value, (fields, by): (Map<String, Value>, Person)) {
  let id = read["id"].as_str().unwrap_or_default().to_owned();
  let revision = read["revision"].as_u64().unwrap_or(1) + 1;
  let edited = authority.edits.entry(id.clone()).or_insert_with(|| Edited {
    revision,
    fields: Map::new(),
    by,
    at: 0,
  });
  edited.fields.extend(fields);
  edited.revision = revision;
  edited.by = by;
  edited.at = Timestamp::now().unix_millis();
  authority.commit(vec![json!({"kind": "task_list"}), json!({"kind": "task", "task": id})]);
}
