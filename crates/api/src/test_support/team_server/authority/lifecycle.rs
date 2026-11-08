//! The stand-in's closing and reopening of a team task (decision D4, `docs/cloud-api.md` "Team authority task
//! lifecycle"). The change is the task patch's `workflow` and `blocked_reason` ([`super::edits`]), decided by the
//! contract's rule ([`LifecycleRequest`]): Done and the manual Blocked by the holder of the task's effective claim or
//! an Admin, Canceled, reopening and Backlog to Todo by an Admin, anything else in the patch an Admin's as before. A
//! member who is not the holder meets `409 not_holder` carrying the task's claim for a change a holder could make, and
//! `403 not_admin` for one only an Admin may. Closing (Done or Canceled) releases the task's own claim and is refused
//! `422 organization {reason:"stop_run_first"}` while its work runs with an accepted report. A task also becomes Done
//! by itself: when its work succeeded and delivers no changes to base (report, outside, or changes without a patch),
//! and when its module's delivery is reported delivered for work that succeeded. Tasks the reads carry no claim for
//! (the v1 tasks, when [`super::TeamServer::authority_tasks`] set none) have no holder; an Admin closes them.

use orchester_core::{resource::Deliverable, task::organization::WorkflowStatus};
use serde::Deserialize;
use serde_json::{Value, json};

use super::{
  Authority, Person, Standing,
  commands::{Refused, module_holder},
  edits,
  listing::task_id,
  work,
};
use crate::{
  TaskChanges, TaskContent,
  v2::{ArtifactRef, LifecycleAction, LifecycleRequest, TaskCaller, completes_on_success},
  wire,
};

/// A refusal's status and body.
pub(super) type Refusal = (u16, Value);

impl super::TeamServer {
  /// The work of the task numbered `number` delivers `delivers`; changes, as a pipeline's default, until said.
  pub fn authority_delivers(&self, number: u32, delivers: Deliverable) {
    self.state.lock().authority.delivers.insert(number, delivers);
  }
}

fn refused(status: u16, error: &str, details: &Value) -> Refusal {
  let mut body = json!({"error": error, "message": "refused"});
  if let (Some(body), Some(details)) = (body.as_object_mut(), details.as_object()) {
    body.extend(details.clone());
  }
  (status, body)
}

/// The caller as a task's lifecycle sees them: an Admin, or the holder of the task's effective claim, its own or else
/// its module's.
fn caller(authority: &Authority, id: &str) -> TaskCaller {
  let own = authority
    .tasks
    .iter()
    .flatten()
    .find(|task| task_id(task) == id)
    .and_then(|task| task.claim.map(|claim| claim.holder));
  let effective = own.or_else(|| module_holder(authority, &json!({"kind": "task", "id": id})));
  TaskCaller {
    admin: authority.standing != Standing::Member,
    holder: effective == Some(Person::You),
  }
}

/// The lifecycle request `changes` make of the task `current` reads as, when the caller may make the patch: otherwise
/// the refusal.
pub(super) fn permitted(
  authority: &Authority,
  current: &Value,
  changes: &TaskChanges,
) -> Result<LifecycleRequest, Refusal> {
  let content = content_of(current).map_err(|_| refused(422, "invalid", &Value::Null))?;
  let request = LifecycleRequest::of(&content, changes);
  let id = current["id"].as_str().unwrap_or_default();
  let caller = caller(authority, id);
  if request.permitted(caller) {
    return Ok(request);
  }
  Err(if request.holder_may() {
    refused(409, "not_holder", &json!({"claim": current["claim"]}))
  } else {
    refused(403, "not_admin", &Value::Null)
  })
}

/// The refusals a permitted patch can still meet: an empty blocked reason, and a close while the work runs.
pub(super) fn checked(
  authority: &Authority,
  current: &Value,
  (request, asked): (&LifecycleRequest, &Value),
) -> Result<(), Refusal> {
  if asked["blocked_reason"]
    .as_str()
    .is_some_and(|reason| reason.trim().is_empty())
  {
    return Err(refused(422, "organization", &json!({"reason": "missing_reason"})));
  }
  let id = current["id"].as_str().unwrap_or_default();
  if request.closes() && running(authority, id) {
    return Err(refused(422, "organization", &json!({"reason": "stop_run_first"})));
  }
  Ok(())
}

/// Whether the task's latest work runs, or waits for a person, with an accepted report: a run exists and has not ended.
fn running(authority: &Authority, id: &str) -> bool {
  let live = |task: &super::AuthorityTask| matches!(task.work, Some("running" | "awaiting_human"));
  authority
    .tasks
    .iter()
    .flatten()
    .any(|task| task_id(task) == id && live(task))
    && authority.reported.iter().any(|reported| reported == id)
}

/// Releases the closed task's own claim; its work that started no run is given up (`failed`), as `claim.release` does.
pub(super) fn release(authority: &mut Authority, id: &str) {
  let Some(task) = authority.tasks.iter_mut().flatten().find(|task| task_id(task) == id) else {
    return;
  };
  task.claim = None;
  if task.work == Some("running") {
    task.work = Some("failed");
  }
}

/// The task's workflow status as the reads give it, an Admin's change laid over.
pub(super) fn workflow(authority: &Authority, id: &str, set: &str) -> WorkflowStatus {
  let name = authority
    .edits
    .get(id)
    .and_then(|edited| edited.field("workflow"))
    .and_then(Value::as_str)
    .unwrap_or(set);
  wire::Workflow::deserialize(Value::from(name)).unwrap_or_default()
}

/// The lifecycle actions in the capabilities of the task `id` that is `set` before any change here.
pub(super) fn actions(authority: &Authority, (id, set): (&str, &str)) -> Vec<Value> {
  let (status, caller) = (workflow(authority, id, set), caller(authority, id));
  let subject = json!({"kind": "task", "id": id});
  LifecycleAction::ALL
    .into_iter()
    .map(|action| {
      let reason = action.denial(status, caller);
      json!({"action": action.as_str(), "subject": subject, "allowed": reason.is_none(), "reason": reason})
    })
    .collect()
}

/// The work of the task `id` reported `succeeded` with `artifacts`: the task is Done by itself when it delivers no
/// changes to base.
pub(super) fn succeeded(authority: &mut Authority, id: &str, artifacts: &Value) -> Result<(), Refused> {
  let artifacts = Vec::<ArtifactRef>::deserialize(artifacts).map_err(|_| (422, "invalid"))?;
  let number = authority
    .tasks
    .iter()
    .flatten()
    .find(|task| task_id(task) == id)
    .map(|task| task.number);
  let delivery = number.and_then(|number| authority.delivers.get(&number)).copied();
  if completes_on_success(delivery.unwrap_or_default(), &artifacts) {
    done_by_itself(authority, id);
  }
  Ok(())
}

/// The module at `index` delivered to base: each of its open tasks whose work succeeded is Done by itself.
pub(super) fn delivered(authority: &mut Authority, index: usize) {
  let ready: Vec<String> = authority
    .tasks
    .iter()
    .flatten()
    .filter(|task| task.module == Some(index) && task.work == Some("succeeded"))
    .map(task_id)
    .collect();
  for id in ready {
    done_by_itself(authority, &id);
  }
}

/// The task `id`, if open, is Done and its claim released, in the name of the person who reported what completed it.
fn done_by_itself(authority: &mut Authority, id: &str) {
  let Some(task) = authority.tasks.iter().flatten().find(|task| task_id(task) == id) else {
    return;
  };
  if workflow(authority, id, task.workflow).is_closed() {
    return;
  }
  let module = task.module.map(work::module_id);
  let edited = authority.edits.get(id);
  let revision = edited.map_or(1, edits::Edited::revision);
  let shipping = edited.and_then(|edited| edited.field("release")).cloned();
  let read = json!({"id": id, "revision": revision, "module": module, "release": shipping});
  release(authority, id);
  let fields = [("workflow".to_owned(), json!("done"))].into_iter().collect();
  edits::change(authority, &read, (fields, Person::You));
}

/// The task's content as its read gives it: the read's fields that are the content's, which refuses any other.
fn content_of(read: &Value) -> Result<TaskContent, serde_json::Error> {
  const CONTENT: [&str; 12] = [
    "title",
    "brief",
    "criteria",
    "pipeline",
    "limits",
    "workflow",
    "priority",
    "tags",
    "blocking",
    "blocked_reason",
    "module",
    "release",
  ];
  let fields = CONTENT.iter().map(|field| ((*field).to_owned(), read[*field].clone()));
  serde_json::from_value(Value::Object(fields.collect()))
}
