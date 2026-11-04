//! The fake team server's module delivery commands (S7c), decided in the order the team server's
//! `docs/module-branches.md` gives: a task subject is `409 transition_denied`, an unknown module `404`, then each
//! command's own checks. `delivery.authorize` is an Admin's and keeps an authorization (`effects:[]`);
//! `delivery.request` needs the module's claim held in the caller's session, the module's branch, no requested delivery
//! of the claim not reported yet, and an unused authorization of exactly this delivery or the automatic merge
//! (`403 delivery_not_authorized` otherwise), and answers with `deliver_module`; `delivery.report` ends the requested
//! delivery of exactly these commits as the merge went. Every answer is `{kind:"delivery", delivery, effects}`.

#![allow(dead_code, reason = "each test binary uses some of the server")]

use orchester_core::id::ClaimId;
use serde_json::{Value, json};

use crate::{
  team_commands::Refused,
  team_server::{ADMIN, Delivery, MODULE_BRANCH, State, holds_module},
};

/// The module the command's subject (or its authority's) names, after the checks every delivery command makes first.
fn module_subject(state: &State, subject: &Value) -> Result<(), Refused> {
  if subject["kind"] == "task" {
    return Err((409, "transition_denied"));
  }
  if subject["id"].as_str() != Some(state.module.to_string().as_str()) {
    return Err((404, "not_found"));
  }
  Ok(())
}

fn claim(value: &Value) -> ClaimId {
  serde_json::from_value(value.clone()).expect("a claim id")
}

fn text(value: &Value) -> String {
  value.as_str().expect("a string").to_owned()
}

/// `delivery.authorize {subject, claim_id, branch, source_commit, base_commit}` (Admin).
pub fn authorize(state: &mut State, (token, operation): (&str, &str), command: &Value) -> Result<Value, Refused> {
  if token != ADMIN {
    return Err((403, "forbidden"));
  }
  module_subject(state, &command["subject"])?;
  let claim = claim(&command["claim_id"]);
  if state.module_claim.as_ref().is_none_or(|current| current.id != claim) {
    return Err((409, "claim_lost"));
  }
  if command["branch"] != MODULE_BRANCH {
    return Err((409, "transition_denied"));
  }
  state.deliveries.push(Delivery {
    authorization: Some(operation.to_owned()),
    claim,
    branch: MODULE_BRANCH.to_owned(),
    source_commit: text(&command["source_commit"]),
    base_commit: text(&command["base_commit"]),
    state: "authorized".to_owned(),
    revision: 1,
    merge_commit: None,
  });
  let delivery = state.deliveries.last().expect("kept").clone();
  Ok(answer(state, &delivery, &[]))
}

/// `delivery.request {authority, authorization_operation_id, branch, source_commit, base_commit}`.
pub fn request(state: &mut State, token: &str, command: &Value) -> Result<Value, Refused> {
  let authority = &command["authority"];
  module_subject(state, &authority["subject"])?;
  let claim = claim(&authority["claim_id"]);
  if !holds_module(state, token, claim) || !authority["work_id"].is_null() {
    return Err((409, "claim_lost"));
  }
  if command["branch"] != MODULE_BRANCH {
    return Err((409, "transition_denied"));
  }
  if state
    .deliveries
    .iter()
    .any(|delivery| delivery.claim == claim && delivery.state == "requested")
  {
    return Err((409, "transition_denied"));
  }
  let (source, base) = (text(&command["source_commit"]), text(&command["base_commit"]));
  let index = match command["authorization_operation_id"].as_str() {
    Some(cited) => state
      .deliveries
      .iter()
      .position(|delivery| {
        delivery.authorization.as_deref() == Some(cited)
          && delivery.state == "authorized"
          && delivery.claim == claim
          && delivery.source_commit == source
          && delivery.base_commit == base
      })
      .ok_or((403, "delivery_not_authorized"))?,
    None if state.automatic_merge => {
      state.deliveries.push(Delivery {
        authorization: None,
        claim,
        branch: MODULE_BRANCH.to_owned(),
        source_commit: source.clone(),
        base_commit: base.clone(),
        state: "authorized".to_owned(),
        revision: 0,
        merge_commit: None,
      });
      state.deliveries.len() - 1
    }
    None => return Err((403, "delivery_not_authorized")),
  };
  let delivery = &mut state.deliveries[index];
  delivery.state = "requested".to_owned();
  delivery.revision += 1;
  let delivery = delivery.clone();
  let effect = json!({"kind": "deliver_module", "module_id": state.module, "branch": MODULE_BRANCH,
    "source_commit": source, "base_commit": base});
  Ok(answer(state, &delivery, &[effect]))
}

/// `delivery.report {authority, source_commit, base_commit, outcome}`.
pub fn report(state: &mut State, token: &str, command: &Value) -> Result<Value, Refused> {
  let authority = &command["authority"];
  module_subject(state, &authority["subject"])?;
  let claim = claim(&authority["claim_id"]);
  if !holds_module(state, token, claim) {
    return Err((409, "claim_lost"));
  }
  let (source, base) = (text(&command["source_commit"]), text(&command["base_commit"]));
  let delivery = state
    .deliveries
    .iter_mut()
    .find(|delivery| {
      delivery.claim == claim
        && delivery.state == "requested"
        && delivery.source_commit == source
        && delivery.base_commit == base
    })
    .ok_or((409, "transition_denied"))?;
  let outcome = &command["outcome"];
  match outcome["kind"].as_str() {
    Some("delivered") => {
      delivery.state = "delivered".to_owned();
      delivery.merge_commit = Some(text(&outcome["merge_commit"]));
    }
    Some("failed") => delivery.state = "failed".to_owned(),
    other => panic!("a delivery outcome: {other:?}"),
  }
  delivery.revision += 1;
  let delivery = delivery.clone();
  Ok(answer(state, &delivery, &[]))
}

/// `{kind:"delivery", delivery: ModuleDeliveryRead, effects}`.
fn answer(state: &State, delivery: &Delivery, effects: &[Value]) -> Value {
  json!({"kind": "delivery", "effects": effects, "delivery": {
    "subject": {"kind": "module", "id": state.module}, "claim_id": delivery.claim,
    "authorization_operation_id": delivery.authorization, "branch": delivery.branch,
    "source_commit": delivery.source_commit, "base_commit": delivery.base_commit, "state": delivery.state,
    "revision": delivery.revision
  }})
}
