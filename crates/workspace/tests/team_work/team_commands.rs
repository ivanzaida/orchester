//! The fake team server's `POST /api/v2/projects/{p}/commands` and `GET …/operations/{operation}` for authorized work,
//! as the team server answers them (its `docs/operation-receipts.md`, `claim-authority.md`, `module-branches.md` and
//! `work-reports.md`): every operation's receipt or refusal kept by session and operation id with its request, the same
//! request answered again with it (`200`, `201` the first time), another under the same id `409 operation_mismatch`.
//! `claim.acquire` and `claim.take_over` issue a new claim id for the caller's session; `work.start` needs the claim's
//! holder in its session, the task's and module's expected revisions, and no running work under the claim, and answers
//! the work with its `start` effect; `work.report` is decided in the server's order: a known event answers as it did,
//! then fencing (`claim_lost`, `session_mismatch`), ordering per work and run (`sequence_regressed`), the newest run,
//! and the outcome's state. The module's delivery commands (S7c) are decided in the order its `module-branches.md`
//! gives them ([`crate::team_deliveries`]).

#![allow(dead_code, reason = "each test binary uses some of the server")]

use orchester_core::id::{ClaimId, WorkId};
use serde_json::{Value, json};

use crate::{
  http::Request,
  team_deliveries,
  team_server::{Claim, MEMBER, OTHER, State, Work, build_bundle, claim_read, holds, refusal, subject_claim_read},
};

pub fn command(state: &mut State, request: &Request) -> (u16, Value) {
  let body = request.json();
  let operation = body["operation_id"].as_str().expect("an operation id").to_owned();
  let identity = json!({ "expected": body["expected"], "command": body["command"] });
  let key = (request.token.clone(), operation.clone());
  match state.operations.get(&key) {
    Some((known, status, answer)) if *known == identity => (if *status == 201 { 200 } else { *status }, answer.clone()),
    Some(_) => (409, refusal(Some(&operation), "operation_mismatch")),
    None => {
      let Some((status, answer)) = carry_out(state, &request.token, &body) else {
        return (400, json!({"error": "bad_request", "message": "refused"}));
      };
      state.operations.insert(key, (identity, status, answer.clone()));
      (status, answer)
    }
  }
}

/// The command carried out, or `None` for a kind the server does not have.
fn carry_out(state: &mut State, token: &str, body: &Value) -> Option<(u16, Value)> {
  let operation = body["operation_id"].as_str().expect("an operation id");
  let command = &body["command"];
  let result = match command["kind"].as_str().expect("a kind") {
    "claim.acquire" => acquire(state, token, command),
    "claim.take_over" => take_over(state, token, command),
    "claim.release" => release(state, token, command),
    "work.start" => start(state, token, body),
    "work.report" => {
      state.reports_sent += 1;
      report(state, token, command)
    }
    "delivery.authorize" => team_deliveries::authorize(state, (token, operation), command),
    "delivery.request" => {
      state.delivery_requests += 1;
      team_deliveries::request(state, token, command)
    }
    "delivery.report" => {
      state.delivery_reports += 1;
      team_deliveries::report(state, token, command)
    }
    _ => return None,
  };
  Some(match result {
    Ok(result) => {
      state.cursor += 1;
      (
        201,
        json!({"operation_id": operation, "state": "committed", "result": result, "cursor": state.cursor.to_string()}),
      )
    }
    Err((status, code)) => (status, refusal(Some(operation), code)),
  })
}

pub type Refused = (u16, &'static str);

fn claimed(state: &State, claim: ClaimId) -> Value {
  json!({"kind": "claim", "subject": {"kind": "task", "id": state.task}, "claim": claim_read(state, claim),
    "children": [], "effects": []})
}

fn acquire(state: &mut State, token: &str, command: &Value) -> Result<Value, Refused> {
  if command["subject"]["kind"] == "module" {
    return acquire_module(state, token, command);
  }
  assert_eq!(command["subject"]["id"].as_str(), Some(state.task.to_string().as_str()));
  match &state.claim {
    Some(claim) if claim.token == token => Ok(claimed(state, claim.id)),
    Some(_) => Err((409, "claim_conflict")),
    None => {
      let id = ClaimId::generate();
      state.claim = Some(Claim {
        id,
        token: token.to_owned(),
      });
      Ok(claimed(state, id))
    }
  }
}

/// The module's claim for the caller's session; its tasks' claims are not modelled here.
fn acquire_module(state: &mut State, token: &str, command: &Value) -> Result<Value, Refused> {
  assert_eq!(
    command["subject"]["id"].as_str(),
    Some(state.module.to_string().as_str())
  );
  let id = match &state.module_claim {
    Some(claim) if claim.token == token => claim.id,
    Some(_) => return Err((409, "claim_conflict")),
    None => {
      let id = ClaimId::generate();
      state.module_claim = Some(Claim {
        id,
        token: token.to_owned(),
      });
      id
    }
  };
  let subject = json!({"kind": "module", "id": state.module});
  Ok(
    json!({"kind": "claim", "claim": subject_claim_read(&subject, id), "subject": subject,
    "children": [], "effects": []}),
  )
}

/// Takes over the claim the caller observed: a new claim id for the caller's session fences the former one's works.
fn take_over(state: &mut State, token: &str, command: &Value) -> Result<Value, Refused> {
  let observed: ClaimId = serde_json::from_value(command["observed_claim_id"].clone()).expect("a claim id");
  if state.claim.as_ref().is_none_or(|claim| claim.id != observed) {
    return Err((409, "claim_conflict"));
  }
  let id = ClaimId::generate();
  state.claim = Some(Claim {
    id,
    token: token.to_owned(),
  });
  Ok(claimed(state, id))
}

/// Releases the task's claim the authority names, the caller's own.
fn release(state: &mut State, token: &str, command: &Value) -> Result<Value, Refused> {
  let authority = &command["authority"];
  let claim: ClaimId = serde_json::from_value(authority["claim_id"].clone()).expect("a claim id");
  if !holds(state, token, claim) {
    return Err((409, "claim_lost"));
  }
  state.claim = None;
  Ok(json!({"kind": "released", "subject": authority["subject"], "released_claim_id": claim, "effects": []}))
}

fn start(state: &mut State, token: &str, body: &Value) -> Result<Value, Refused> {
  let kinds: Vec<&str> = body["expected"]
    .as_array()
    .expect("expected revisions")
    .iter()
    .filter_map(|expected| expected["kind"].as_str())
    .collect();
  if !(kinds.contains(&"task") && kinds.contains(&"module")) {
    return Err((422, "invalid"));
  }
  let authority = &body["command"]["authority"];
  let claim: ClaimId = serde_json::from_value(authority["claim_id"].clone()).expect("a claim id");
  if !holds(state, token, claim) || !authority["work_id"].is_null() {
    return Err((409, "claim_lost"));
  }
  if state
    .works
    .iter()
    .any(|work| work.claim == claim && work.state == "running")
  {
    return Err((409, "transition_denied"));
  }
  let bundle = build_bundle(state, claim, body["command"]["base_commit"].as_str().expect("a commit"));
  let work = Work {
    id: WorkId::generate(),
    claim,
    bundle: bundle["sha256"].as_str().expect("a hash").to_owned(),
    state: "running".to_owned(),
    revision: 1,
    runs: Vec::new(),
  };
  state.works.push(work.clone());
  state.works_made += 1;
  Ok(json!({
    "kind": "work", "work": work_read(state, &work), "bundle": bundle,
    "effects": [{"kind": "start", "work_id": work.id, "bundle": bundle}]
  }))
}

/// `work.report`, in the server's order.
fn report(state: &mut State, token: &str, command: &Value) -> Result<Value, Refused> {
  let event = command["event_id"].as_str().expect("an event id").to_owned();
  if let Some((known, answer)) = state.events.get(&event) {
    return if known == command {
      Ok(answer.clone())
    } else {
      Err((422, "invalid"))
    };
  }
  if token != MEMBER && token != OTHER {
    return Err((403, "membership_revoked"));
  }
  let claim: ClaimId = serde_json::from_value(command["authority"]["claim_id"].clone()).expect("a claim id");
  let work_id: WorkId = serde_json::from_value(command["authority"]["work_id"].clone()).expect("a work id");
  let latest = state
    .works
    .iter()
    .rev()
    .find(|work| work.claim == claim)
    .map(|work| work.id);
  let current = state.claim.as_ref().is_some_and(|current| current.id == claim);
  if !current || latest != Some(work_id) {
    return Err((409, "claim_lost"));
  }
  if !holds(state, token, claim) {
    return Err((409, "session_mismatch"));
  }
  let run = command["run_id"].as_str().expect("a run id").to_owned();
  let sequence = command["sequence"].as_u64().expect("a sequence");
  let last = state.sequences.get(&(work_id, run.clone())).copied();
  if last.is_some_and(|last| sequence <= last) {
    return Err((409, "sequence_regressed"));
  }
  state.sequences.insert((work_id, run.clone()), sequence);
  let outcome = &command["outcome"];
  let work = state
    .works
    .iter_mut()
    .find(|work| work.id == work_id)
    .expect("the work");
  if !work.runs.contains(&run) {
    work.runs.push(run);
  }
  work.state = match (outcome["kind"].as_str(), outcome["reason"].as_str()) {
    (Some("progress"), _) => "running",
    (Some("succeeded"), _) => "succeeded",
    (Some("failed"), Some("limit_reached")) => "paused",
    (Some("failed"), _) => "failed",
    (Some("cancelled"), _) => "cancelled",
    other => panic!("an outcome: {other:?}"),
  }
  .to_owned();
  work.revision += 1;
  let work = work.clone();
  if work.state == "cancelled" {
    state.claim = None;
  }
  let kind = outcome["kind"].as_str().expect("a kind").to_owned();
  state.accepted.push((sequence, kind));
  let answer = json!({"kind": "work", "work": work_read(state, &work),
    "bundle": {"sha256": work.bundle, "schema_version": 1}, "effects": []});
  state.events.insert(event, (command.clone(), answer.clone()));
  Ok(answer)
}

fn work_read(state: &State, work: &Work) -> Value {
  json!({"id": work.id, "subject": {"kind": "task", "id": state.task}, "claim_id": work.claim,
    "revision": work.revision, "state": work.state, "runs": work.runs,
    "bundle": {"sha256": work.bundle, "schema_version": 1}, "created_at": 1, "updated_at": 2})
}

/// `GET …/operations/{operation}`: the kept receipt or refusal, or `404 not_found`.
pub fn operation_read(state: &State, token: &str, operation: &str) -> (u16, Value) {
  match state.operations.get(&(token.to_owned(), operation.to_owned())) {
    Some((_, 201, receipt)) => (
      200,
      json!({"operation_id": operation, "state": "committed", "response": receipt}),
    ),
    Some((_, _, error)) => (
      200,
      json!({"operation_id": operation, "state": "rejected", "error": error}),
    ),
    None => (404, refusal(Some(operation), "not_found")),
  }
}
