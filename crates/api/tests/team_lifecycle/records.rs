//! How and by whom a task was closed, and who set its blocked reason, on the stand-in team server: the `closed` and
//! `blocked` of a task read are set with the change that makes them true, in its transaction, and cleared with the
//! change that ends them.

use orchester_api::{
  test_support::team_server::{Person, Standing},
  v2::{ClosedHow, TaskRead},
};
use orchester_core::resource::Deliverable;
use serde_json::{Value, json};

use crate::team::{OLHA, Team, YOU, claim_id, module, patch_artifact, report_artifact, run, task};

const MODULE: &str = "0191c7c0-0000-7000-8000-000000200000";

fn typed(read: &Value) -> TaskRead {
  serde_json::from_value(read.clone()).expect("a task read")
}

fn who(record: &Value) -> &str {
  record["by"]["display_name"].as_str().expect("a person")
}

#[test]
fn team_lifecycle_a_task_nobody_recorded_says_nothing_of_its_closing() {
  let team = Team::start(Standing::Admin, vec![task(1, "done", None), task(2, "todo", None)]);
  for number in [1, 2] {
    let read = team.task(number);
    assert!(read.get("closed").is_none() && read.get("blocked").is_none(), "{read}");
    let read = typed(&read);
    assert_eq!((read.closed, read.blocked), (None, None));
  }
}

#[test]
fn team_lifecycle_a_person_who_closes_a_task_is_recorded_whoever_held_it_and_edits_after_leave_it() {
  let team = Team::start(
    Standing::Admin,
    vec![task(1, "in_progress", Some(OLHA)), task(2, "todo", None)],
  );
  assert_eq!(team.patch(1, &json!({"workflow": "done"})).0, 200);
  let done = team.task(1);
  assert_eq!(done["closed"]["how"]["kind"], "person");
  assert_eq!(
    who(&done["closed"]["how"]),
    "Bohdan Koval",
    "an Admin closed Olha's task: the Admin is recorded, not the holder"
  );
  assert_eq!(done["closed"]["at"], done["updated"]["at"]);
  assert!(matches!(
    typed(&done).closed.map(|closed| closed.how),
    Some(ClosedHow::Person { .. })
  ));

  assert_eq!(team.patch(1, &json!({"title": "Renamed"})).0, 200);
  let renamed = team.task(1);
  assert_eq!(
    renamed["closed"], done["closed"],
    "an edit of a closed task is not a close"
  );
  assert!(renamed["revision"].as_u64() > done["revision"].as_u64());

  assert_eq!(team.patch(1, &json!({"workflow": "canceled"})).0, 200);
  let canceled = team.task(1);
  assert_eq!(canceled["closed"]["how"]["kind"], "person");
  assert_eq!(
    canceled["closed"]["at"], canceled["updated"]["at"],
    "Canceled is a close of its own"
  );

  assert_eq!(team.patch(1, &json!({"workflow": "todo"})).0, 200);
  assert!(team.task(1)["closed"].is_null(), "a reopened task is not closed");
  assert_eq!(team.patch(1, &json!({"workflow": "canceled"})).0, 200);
  assert_eq!(team.task(1)["closed"]["how"]["kind"], "person", "and closes anew");

  assert_eq!(team.patch(2, &json!({"workflow": "in_review"})).0, 200);
  assert!(
    team.task(2)["closed"].is_null(),
    "an open task that moves is not closed"
  );
}

#[test]
fn team_lifecycle_the_holder_who_marks_their_task_done_is_recorded() {
  let team = Team::start(Standing::Member, vec![task(1, "in_progress", Some(YOU))]);
  assert_eq!(team.patch(1, &json!({"workflow": "done"})).0, 200);
  let done = team.task(1);
  assert_eq!(who(&done["closed"]["how"]), "Bohdan Koval");
  assert_eq!(done["closed"]["how"]["by"]["id"], done["updated"]["by"]["id"]);
}

#[test]
fn team_lifecycle_work_that_delivers_nothing_to_base_closes_its_task_with_its_run() {
  let tasks = vec![task(1, "in_progress", Some(YOU)), task(2, "in_progress", Some(YOU))];
  let team = Team::start(Standing::Member, tasks);
  team.server.authority_delivers(1, Deliverable::Report);
  team.server.authority_delivers(2, Deliverable::ChangesOrReport);
  run(&team, 1, &json!({"kind": "succeeded"}), &report_artifact());
  run(&team, 2, &json!({"kind": "succeeded"}), &patch_artifact());
  let done = team.task(1);
  assert_eq!(done["workflow"], "done");
  let work = "0191c7c0-0000-7000-8000-000000400001";
  assert_eq!(
    done["closed"],
    json!({"how": {"kind": "run", "work": work}, "at": done["updated"]["at"]}),
    "the claim is released, so the read no longer carries the work; the record names it"
  );
  assert!(
    team.task(2)["closed"].is_null(),
    "a changes' success closes nothing yet"
  );
}

#[test]
fn team_lifecycle_an_accepted_delivery_closes_the_succeeded_work_with_its_module() {
  let mut tasks = vec![
    task(1, "in_progress", Some(YOU)),
    task(2, "in_progress", Some(YOU)),
    task(3, "in_progress", Some(YOU)),
  ];
  for task in &mut tasks {
    task.module = Some(0);
  }
  let team = Team::start(Standing::Admin, tasks);
  team.server.authority_modules(vec![module(Some(YOU))]);
  team.server.automatic_merge(true);
  run(&team, 1, &json!({"kind": "succeeded"}), &patch_artifact());
  run(&team, 3, &json!({"kind": "succeeded"}), &patch_artifact());
  assert_eq!(team.patch(3, &json!({"workflow": "canceled"})).0, 200);
  let canceled = team.task(3)["closed"].clone();

  let authority = json!({"subject": {"kind": "module", "id": MODULE}, "claim_id": claim_id(0, 0x800), "work_id": null});
  let (source, base) = ("c".repeat(40), "d".repeat(40));
  let request = json!({"kind": "delivery.request", "authority": authority, "authorization_operation_id": null,
    "branch": "module/payments", "source_commit": source, "base_commit": base});
  assert_eq!(team.command(&request).0, 201);
  let report = json!({"kind": "delivery.report", "authority": authority, "source_commit": source,
    "base_commit": base, "outcome": {"kind": "delivered", "merge_commit": "e".repeat(40)}});
  assert_eq!(team.command(&report).0, 201);

  let done = team.task(1);
  assert_eq!(
    done["closed"],
    json!({"how": {"kind": "delivery", "module": MODULE, "module_name": "Payments"}, "at": done["updated"]["at"]})
  );
  assert!(matches!(
    typed(&done).closed.map(|closed| closed.how),
    Some(ClosedHow::Delivery { .. })
  ));
  assert!(
    team.task(2)["closed"].is_null(),
    "work that did not succeed is not delivered"
  );
  assert_eq!(
    team.task(3)["closed"],
    canceled,
    "a task already closed keeps how it was"
  );
}

#[test]
fn team_lifecycle_who_blocked_a_task_is_recorded_until_the_reason_is_cleared() {
  let team = Team::start(Standing::Admin, vec![task(1, "in_progress", Some(YOU))]);
  assert_eq!(team.patch(1, &json!({"blocked_reason": "waiting for the API"})).0, 200);
  let blocked = team.task(1);
  assert_eq!(who(&blocked["blocked"]), "Bohdan Koval");
  assert_eq!(blocked["blocked"]["at"], blocked["updated"]["at"]);
  assert_eq!(
    typed(&blocked).blocked.map(|record| record.at),
    blocked["blocked"]["at"].as_i64()
  );

  team.server.task_changed_first(1, Person::Named("olha"), "high");
  assert_eq!(
    team.patch(1, &json!({"title": "Renamed"})).0,
    409,
    "olha's edit came first"
  );
  let edited = team.task(1);
  assert_eq!(edited["updated"]["by"]["display_name"], "olha");
  assert_eq!(
    edited["blocked"], blocked["blocked"],
    "later edits of the task, by others, are not the block"
  );

  assert_eq!(team.patch(1, &json!({"blocked_reason": "waiting for the API"})).0, 200);
  assert_eq!(
    team.task(1)["blocked"],
    blocked["blocked"],
    "the same reason is no change"
  );

  assert_eq!(team.patch(1, &json!({"workflow": "done"})).0, 200);
  assert_eq!(
    team.task(1)["blocked"],
    blocked["blocked"],
    "closing does not clear the reason"
  );

  assert_eq!(team.patch(1, &json!({"blocked_reason": null})).0, 200);
  assert!(
    team.task(1)["blocked"].is_null(),
    "clearing the reason clears who set it"
  );
  assert!(team.task(1)["blocked_reason"].is_null());
}

#[test]
fn team_lifecycle_the_holder_who_blocks_their_task_is_recorded_and_a_new_reason_is_a_new_record() {
  let team = Team::start(Standing::Member, vec![task(1, "in_progress", Some(YOU))]);
  assert_eq!(team.patch(1, &json!({"blocked_reason": "first"})).0, 200);
  assert_eq!(who(&team.task(1)["blocked"]), "Bohdan Koval");
  assert_eq!(team.patch(1, &json!({"blocked_reason": "second"})).0, 200);
  let second = team.task(1);
  assert_eq!(second["blocked"]["at"], second["updated"]["at"]);
  assert_eq!(
    team.patch(1, &json!({"workflow": "done", "blocked_reason": "last"})).0,
    200
  );
  let both = team.task(1);
  assert_eq!(both["closed"]["at"], both["blocked"]["at"], "one patch, one moment");
}
