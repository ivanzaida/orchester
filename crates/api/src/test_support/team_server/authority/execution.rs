//! The stand-in's authorized work on a task (`docs/claim-authority.md` and `work-reports.md` of the platform):
//! `work.start` under the person's current claim of the task, which makes its work running with a `start` effect;
//! `work.report` of a run carrying that work out, which moves the work's state as the report's outcome says; and `POST
//! heartbeat`, which renews the claims of the work that runs (D1). A task's work has the id its reads give it, made
//! from its number, so a start and a read name the same work.

use serde_json::{Value, json};

use super::{Authority, AuthorityClaim, AuthorityTask, Person, commands::Refused, listing};

/// `work.start` of the task the authority names, under the person's current claim of it and with no work yet.
pub(super) fn start(authority: &mut Authority, command: &Value) -> Result<Value, Refused> {
  let named = &command["authority"];
  if named["subject"]["kind"] != "task" {
    return Err((422, "invalid"));
  }
  if !named["work_id"].is_null() {
    return Err((409, "claim_lost"));
  }
  let (task, claim) = held(authority, named)?;
  if task.work == Some("running") {
    return Err((409, "transition_denied"));
  }
  task.work = Some("running");
  let work = listing::work_read((&listing::task_id(task), task.number), claim, "running");
  let bundle = work["bundle"].clone();
  Ok(json!({"kind": "work", "work": work, "bundle": bundle,
    "effects": [{"kind": "start", "work_id": work["id"], "bundle": bundle}]}))
}

/// `work.report` of the run `run_id` under the task's current claim and work: the work as the outcome leaves it.
pub(super) fn report(authority: &mut Authority, command: &Value) -> Result<Value, Refused> {
  let named = &command["authority"];
  let (task, claim) = held(authority, named)?;
  let id = listing::task_id(task);
  let current = listing::work_read((&id, task.number), claim, "running");
  if task.work.is_none() || named["work_id"] != current["id"] {
    return Err((409, "claim_lost"));
  }
  let outcome = &command["outcome"];
  let state = match (outcome["kind"].as_str(), outcome["reason"].as_str()) {
    (Some("progress"), _) => "running",
    (Some("succeeded"), _) => "succeeded",
    (Some("failed"), Some("limit_reached")) => "paused",
    (Some("failed"), _) => "failed",
    (Some("cancelled"), _) => "cancelled",
    _ => return Err((422, "invalid")),
  };
  task.work = Some(state);
  if state == "cancelled" {
    task.claim = None;
  }
  let mut work = listing::work_read((&id, task.number), claim, state);
  work["runs"] = json!([command["run_id"]]);
  let bundle = work["bundle"].clone();
  Ok(json!({"kind": "work", "work": work, "bundle": bundle, "effects": []}))
}

/// `POST …/heartbeat {authorities}`: renews each named claim the person holds whose work runs; the others are lost.
/// Work that waits for a person refuses the whole heartbeat, `409 decision_required [D1]`, and work in any other state
/// but running `409 transition_denied`.
pub(super) fn heartbeat(authority: &mut Authority, body: &str) -> (u16, Value) {
  let Ok(body) = serde_json::from_str::<Value>(body) else {
    return (422, json!({"code": "invalid", "details": {}}));
  };
  let (mut renewed, mut lost) = (Vec::new(), Vec::new());
  for named in body["authorities"].as_array().into_iter().flatten() {
    let Ok((task, claim)) = held(authority, named) else {
      lost.push(named.clone());
      continue;
    };
    let id = listing::task_id(task);
    let work = listing::work_read((&id, task.number), claim, "running");
    if !named["work_id"].is_null() && named["work_id"] != work["id"] {
      lost.push(named.clone());
      continue;
    }
    match task.work {
      Some("running") => {}
      Some("awaiting_human") => {
        return (
          409,
          json!({"code": "decision_required", "details": {"decisions": ["D1"]}}),
        );
      }
      _ => return (409, json!({"code": "transition_denied", "details": {}})),
    }
    let fresh = AuthorityClaim {
      renewed_minutes_ago: 0,
      stale: false,
      ..claim
    };
    task.claim = Some(fresh);
    renewed.push(fresh.wire(&json!({"kind": "task", "id": id}), task.number));
  }
  (200, json!({"renewed": renewed, "lost": lost}))
}

/// The task the authority `named` names, with the person's claim of it when its id is the one named; `409 claim_lost`
/// otherwise.
fn held<'a>(authority: &'a mut Authority, named: &Value) -> Result<(&'a mut AuthorityTask, AuthorityClaim), Refused> {
  let tasks = authority.tasks.as_mut().ok_or((404, "not_found"))?;
  let task = tasks
    .iter_mut()
    .find(|task| named["subject"]["id"] == listing::task_id(task))
    .ok_or((404, "not_found"))?;
  match task.claim {
    Some(claim) if claim.holder == Person::You && named["claim_id"] == claim.id(task.number) => Ok((task, claim)),
    _ => Err((409, "claim_lost")),
  }
}
