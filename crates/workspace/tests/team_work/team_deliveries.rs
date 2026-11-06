//! The fake team server's module delivery commands (S7c and S12), decided in the order the team server's
//! `docs/module-branches.md` gives: a task subject is `409 transition_denied`, an unknown module `404`, then each
//! command's own checks. `delivery.authorize` is an Admin's: of a request that waits for the Admin with exactly these
//! commits it is that request's authorization, which then waits for the holder; otherwise it is kept for a request to
//! cite (`effects:[]` either way). `delivery.request` needs the module's claim held in the caller's session and the
//! module's branch; citing an unused authorization of exactly this delivery, or with the automatic merge on, it answers
//! with `deliver_module`; citing nothing with the automatic merge off, it waits for the Admin (`effects:[]`). Asked
//! again for the open delivery of the claim, the same commits get the merge once the Admin authorized them, are
//! answered as they stand while they wait, and other commits, or a delivery whose merge was given, are `409
//! transition_denied`. `delivery.report` ends the requested delivery of exactly these commits as the merge went; one
//! that still waits gives up as `failed`, and `delivered` is `403 delivery_not_authorized`. Every answer is
//! `{kind:"delivery", delivery, effects}`.

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

/// A delivery of `claim` and the commits `(source, base)`, `state`, authorized by `authorization`.
fn kept(claim: ClaimId, (source, base): (&str, &str), (state, authorization): (&str, Option<&str>)) -> Delivery {
  Delivery {
    authorization: authorization.map(str::to_owned),
    claim,
    branch: MODULE_BRANCH.to_owned(),
    source_commit: source.to_owned(),
    base_commit: base.to_owned(),
    state: state.to_owned(),
    revision: 1,
    merge_commit: None,
    waits: None,
  }
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
  let (source, base) = (text(&command["source_commit"]), text(&command["base_commit"]));
  let waiting = state.deliveries.iter_mut().find(|delivery| {
    delivery.claim == claim
      && delivery.waits == Some("admin")
      && delivery.source_commit == source
      && delivery.base_commit == base
  });
  let delivery = match waiting {
    Some(waiting) => {
      waiting.authorization = Some(operation.to_owned());
      waiting.waits = Some("holder");
      waiting.revision += 1;
      waiting.clone()
    }
    None => {
      let authorized = kept(claim, (&source, &base), ("authorized", Some(operation)));
      state.deliveries.push(authorized.clone());
      authorized
    }
  };
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
  let (source, base) = (text(&command["source_commit"]), text(&command["base_commit"]));
  let cited = command["authorization_operation_id"].as_str();
  let open = state
    .deliveries
    .iter()
    .position(|delivery| delivery.claim == claim && delivery.state == "requested");
  let index = match open {
    Some(index) => asked_again(state, index, (&source, &base), cited)?,
    None => match cited {
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
      None => {
        let mut asked = kept(claim, (&source, &base), ("requested", None));
        asked.waits = (!state.automatic_merge).then_some("admin");
        state.deliveries.push(asked);
        state.deliveries.len() - 1
      }
    },
  };
  let delivery = &mut state.deliveries[index];
  if delivery.waits == Some("admin") {
    let delivery = delivery.clone();
    return Ok(answer(state, &delivery, &[]));
  }
  delivery.state = "requested".to_owned();
  delivery.waits = None;
  delivery.revision += 1;
  let delivery = delivery.clone();
  let effect = json!({"kind": "deliver_module", "module_id": state.module, "branch": MODULE_BRANCH,
    "source_commit": source, "base_commit": base});
  Ok(answer(state, &delivery, &[effect]))
}

/// A request while the claim's delivery at `index` is open: the same commits only, not once its merge was given; one
/// that still waits for the Admin cites nothing, and is the automatic merge's when that is on now.
fn asked_again(
  state: &mut State,
  index: usize,
  (source, base): (&str, &str),
  cited: Option<&str>,
) -> Result<usize, Refused> {
  let automatic = state.automatic_merge;
  let open = &mut state.deliveries[index];
  if open.source_commit != source || open.base_commit != base || open.waits.is_none() {
    return Err((409, "transition_denied"));
  }
  match (open.waits, cited) {
    (Some("admin"), Some(_)) => return Err((403, "delivery_not_authorized")),
    (Some("holder"), Some(cited)) if open.authorization.as_deref() != Some(cited) => {
      return Err((403, "delivery_not_authorized"));
    }
    (Some("admin"), None) if automatic => open.waits = None,
    _ => {}
  }
  Ok(index)
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
    Some("delivered") if delivery.waits.is_some() => return Err((403, "delivery_not_authorized")),
    Some("delivered") => {
      delivery.state = "delivered".to_owned();
      delivery.merge_commit = Some(text(&outcome["merge_commit"]));
    }
    Some("failed") => delivery.state = "failed".to_owned(),
    other => panic!("a delivery outcome: {other:?}"),
  }
  delivery.waits = None;
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
