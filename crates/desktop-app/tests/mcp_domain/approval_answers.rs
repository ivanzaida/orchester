//! How each tool request was answered by its run's approval mode and why one came to the person (ORC-346,
//! `docs/design/approval-modes.md`), as `list_approvals` and `get_run` give them: a waiting request's `came_to_you`
//! with its reason code and what the card names with it, and every request of a run with how it was answered.
//! `cargo test -p orchester-app --test mcp_domain approval_mode_answer`.

use orchester_core::{
  run::{CameToPerson, DecidedBy, PersonReason, RequestAnswer, ToolApprovalDecision},
  store::RunStore,
  time::Timestamp,
};
use orchester_store::ProjectDb;
use serde_json::{Value, json};

use super::Served;

#[test]
fn approval_mode_answer_mcp_shows_how_and_why_requests_were_answered() {
  let (mut served, seeded) = Served::seeded("cards");
  let mut db = ProjectDb::open(&seeded.repo).expect("open the project");
  let waiting = db.pending_tool_approvals_all().expect("waiting requests");
  let [outside, reviewed, ..] = waiting.as_slice() else {
    panic!("the cards wait on two requests at least: {waiting:?}");
  };
  let came = CameToPerson {
    reason: PersonReason::Outside,
    detail: Some("/elsewhere/notes.md".to_owned()),
  };
  db.record_came_to_person(outside.run, &outside.id, Some(&came))
    .expect("came to the person");
  let reason = "The reviewer finds it serves the task.";
  db.claim_tool_approval(reviewed.run, &reviewed.id, DecidedBy::Reviewer, Timestamp::now())
    .expect("claimed");
  db.record_request_answer(reviewed.run, &reviewed.id, &RequestAnswer::reviewer(reason))
    .expect("answered");
  db.delivered_tool_approval(reviewed.run, &reviewed.id, ToolApprovalDecision::Approved)
    .expect("delivered");
  drop(db);

  let approvals = served.ok("list_approvals", &json!({ "project": "Kontur" }));
  let listed = approvals["approvals"].as_array().expect("approvals");
  let shown = listed
    .iter()
    .find(|approval| approval["id"] == outside.id.as_str())
    .unwrap_or_else(|| panic!("the request that came to the person: {approvals}"));
  assert_eq!(
    shown["came_to_you"],
    json!({ "reason": "outside", "detail": "/elsewhere/notes.md" }),
    "{shown}"
  );
  assert!(
    listed.iter().all(|approval| approval["id"] != reviewed.id.as_str()),
    "the reviewer's approval no longer waits: {approvals}"
  );

  let requests_of = |served: &mut Served, number: Value| {
    let run = served.ok("get_run", &json!({ "project": "Kontur", "run": number.to_string() }));
    run["requests"]
      .as_array()
      .unwrap_or_else(|| panic!("the run's requests: {run}"))
      .clone()
  };
  let number = shown["run"]["number"].clone();
  let requests = requests_of(&mut served, number);
  let request = requests
    .iter()
    .find(|request| request["id"] == outside.id.as_str())
    .unwrap_or_else(|| panic!("the waiting request: {requests:?}"));
  assert_eq!(request["answered"], Value::Null, "it still waits: {request}");
  assert_eq!(request["came_to_you"]["reason"], "outside", "{request}");

  let reviewed_number = {
    let run = ProjectDb::open(&seeded.repo)
      .expect("open")
      .run(reviewed.run)
      .expect("the run")
      .expect("it exists");
    json!(run.number().get())
  };
  let requests = requests_of(&mut served, reviewed_number);
  let request = requests
    .iter()
    .find(|request| request["id"] == reviewed.id.as_str())
    .unwrap_or_else(|| panic!("the reviewed request: {requests:?}"));
  assert_eq!(
    request["answered"],
    json!({ "kind": "reviewer", "reason": reason }),
    "{request}"
  );
  assert_eq!(request["came_to_you"], Value::Null, "{request}");
}
