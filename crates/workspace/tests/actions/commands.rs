//! The fake team server's `POST /api/v2/projects/{p}/commands` and `GET …/operations/{operation}`, as the team server
//! answers them (its `docs/operation-receipts.md`): every operation's receipt or refusal kept by account and operation
//! id with its request, so the same request under the same id answers the same receipt (`200`, `201` the first time)
//! and another request under it is `409 operation_mismatch`. `task.create` is the Admin's (`403 forbidden`), a claim
//! and a work start every reader's; `work.start` needs the task's and its module's expected revisions
//! (`422 invalid`) and answers its work with the `start` effect. Any other kind is malformed (`400`).

use orchester_core::id::{ClaimId, TaskId, WorkId};
use serde_json::{Value, json};

use crate::{
  http::Request,
  server::{ADMIN, State, Task, api_error, person, shared_task},
};

pub fn command(state: &mut State, request: &Request) -> (u16, Value) {
  let body = request.json();
  let operation = body["operation_id"].as_str().expect("an operation id").to_owned();
  let identity = json!({ "expected": body["expected"], "command": body["command"] });
  let key = (request.token.clone(), operation.clone());
  match state.operations.get(&key) {
    Some((known, status, answer)) if *known == identity => (if *status == 201 { 200 } else { *status }, answer.clone()),
    Some(_) => (409, refusal(&operation, "operation_mismatch")),
    None => {
      let Some((status, answer)) = carry_out(state, &request.token, &body) else {
        return (400, api_error("bad_request"));
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
    "claim.acquire" => claim(command),
    "work.start" => match work(body) {
      Some(work) => work,
      None => return Some((422, refusal(operation, "invalid"))),
    },
    "task.create" if token != ADMIN => return Some((403, refusal(operation, "forbidden"))),
    "task.create" => {
      let task = Task {
        id: state.next_task.take().unwrap_or_else(TaskId::generate),
        number: u32::try_from(state.tasks.len()).expect("a few tasks") + 1,
        title: command["content"]["title"].as_str().expect("a title").to_owned(),
        revision: 1,
      };
      state.tasks.push(task.clone());
      state.created += 1;
      json!({"kind": "task", "task": shared_task(&task), "effects": []})
    }
    _ => return None,
  };
  state.cursor += 1;
  let receipt =
    json!({"operation_id": operation, "state": "committed", "result": result, "cursor": state.cursor.to_string()});
  Some((201, receipt))
}

/// A claim of the command's subject: it allows nothing on the caller's computer.
fn claim(command: &Value) -> Value {
  json!({
    "kind": "claim", "subject": command["subject"],
    "claim": {"subject": command["subject"], "id": ClaimId::generate(), "holder": person(), "acquired_at": 1,
      "last_execution_heartbeat_at": 1, "stale_at": 2, "stale": false, "parent_module_claim_id": null},
    "children": [],
    "effects": []
  })
}

/// The task's work under the command's authority, with the start that lets the caller's computer run it; `None` when
/// the expected revisions do not name both the task and its module.
fn work(body: &Value) -> Option<Value> {
  let kinds: Vec<&str> = body["expected"]
    .as_array()
    .expect("expected revisions")
    .iter()
    .filter_map(|expected| expected["kind"].as_str())
    .collect();
  if !(kinds.contains(&"task") && kinds.contains(&"module")) {
    return None;
  }
  let authority = &body["command"]["authority"];
  let (id, bundle) = (
    WorkId::generate(),
    json!({"sha256": "a".repeat(64), "schema_version": 1}),
  );
  Some(json!({
    "kind": "work",
    "work": {"id": id, "subject": authority["subject"], "claim_id": authority["claim_id"], "revision": 1,
      "state": "running", "runs": [], "bundle": bundle, "created_at": 1, "updated_at": 1},
    "bundle": bundle,
    "effects": [{"kind": "start", "work_id": id, "bundle": bundle}]
  }))
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
    None => (
      404,
      json!({"operation_id": operation, "code": "not_found", "details": {}}),
    ),
  }
}

fn refusal(operation: &str, code: &str) -> Value {
  json!({"operation_id": operation, "code": code, "details": {}})
}
