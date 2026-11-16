//! A closed task and a reopened one on the stand-in team server: a closed task can be neither claimed nor started until
//! an Admin reopens it, and a reopened task is Done by itself again only by a work that succeeded after the reopen.

use orchester_api::test_support::team_server::Standing;
use orchester_core::{
  id::{RunId, WorkEventId},
  resource::Deliverable,
};
use serde_json::{Value, json};

use crate::team::{
  Team, YOU, authority, claim_id, denied, module, patch_artifact, report_artifact, run, start, task, task_id, workflow,
};

const MODULE: &str = "0191c7c0-0000-7000-8000-000000200000";

fn claim(number: u32, kind: &str) -> Value {
  json!({"kind": kind, "subject": {"kind": "task", "id": task_id(number)}, "observed_claim_id": claim_id(0, number)})
}

#[test]
fn team_lifecycle_a_closed_task_is_neither_claimed_nor_started_until_reopened() {
  let team = Team::start(
    Standing::Admin,
    vec![
      task(1, "done", None),
      task(2, "canceled", None),
      task(3, "done", Some(YOU)),
    ],
  );
  for number in [1_u32, 2, 3] {
    let read = team.task(u64::from(number));
    for kind in ["claim.acquire", "claim.take_over"] {
      assert_eq!(denied(&read, kind), Some("transition_denied"), "task {number} {kind}");
      let (status, refused) = team.command(&claim(number, kind));
      assert_eq!(
        (status, refused["code"].as_str()),
        (409, Some("transition_denied")),
        "task {number} {kind}"
      );
    }
  }
  assert_eq!(
    denied(&team.task(3), "claim.release"),
    None,
    "the claim a closed task still shows can be let go of"
  );
  let seeded = json!({"kind": "work.start", "authority": authority(3), "base_commit": "b".repeat(40)});
  let (status, refused) = team.command(&seeded);
  assert_eq!((status, refused["code"].as_str()), (409, Some("transition_denied")));
  assert!(team.task(3)["work"].is_null(), "a refused start starts nothing");

  assert_eq!(team.patch(1, &json!({"workflow": "todo"})).0, 200);
  assert_eq!(
    denied(&team.task(1), "claim.acquire"),
    None,
    "reopened, it is free to claim"
  );
  let (status, taken) = team.command(&claim(1, "claim.acquire"));
  assert_eq!(status, 201, "{taken}");
}

#[test]
fn team_lifecycle_a_reopened_task_is_not_done_again_by_a_delivery_of_work_from_before() {
  let mut tasks = vec![task(1, "in_progress", Some(YOU))];
  tasks[0].module = Some(0);
  let team = Team::start(Standing::Admin, tasks);
  team.server.authority_modules(vec![module(Some(YOU))]);
  team.server.automatic_merge(true);
  run(&team, 1, &json!({"kind": "succeeded"}), &patch_artifact());
  assert_eq!(team.patch(1, &json!({"workflow": "done"})).0, 200);
  assert_eq!(team.patch(1, &json!({"workflow": "todo"})).0, 200);
  assert!(
    team.task(1)["work"].is_null(),
    "the work from before does not carry over"
  );

  deliver(&team);
  assert_eq!(
    workflow(&team.task(1)),
    "todo",
    "a delivery marks nothing for work that ended before the reopen"
  );

  let (status, taken) = team.command(&claim(1, "claim.acquire"));
  assert_eq!(status, 201, "{taken}");
  let mut named = authority(1);
  named["claim_id"] = taken["result"]["claim"]["id"].clone();
  let start = json!({"kind": "work.start", "authority": named, "base_commit": "b".repeat(40)});
  let (status, started) = team.command(&start);
  assert_eq!(status, 201, "{started}");
  named["work_id"] = started["result"]["work"]["id"].clone();
  let report = json!({"kind": "work.report", "authority": named, "run_id": RunId::generate(),
    "event_id": WorkEventId::generate(), "sequence": 1, "outcome": {"kind": "succeeded"},
    "artifacts": patch_artifact()});
  assert_eq!(team.command(&report).0, 201);
  assert_eq!(workflow(&team.task(1)), "todo", "its success alone marks nothing");
  deliver(&team);
  assert_eq!(
    workflow(&team.task(1)),
    "done",
    "the new work, delivered, marks it Done"
  );
}

/// The module's delivery to base, requested and reported delivered.
fn deliver(team: &Team) {
  let module_authority = json!({"subject": {"kind": "module", "id": MODULE}, "claim_id": claim_id(0, 0x800),
    "work_id": null});
  let (source, base) = ("c".repeat(40), "d".repeat(40));
  let request = json!({"kind": "delivery.request", "authority": module_authority, "authorization_operation_id": null,
    "branch": "module/payments", "source_commit": source, "base_commit": base});
  assert_eq!(team.command(&request).0, 201);
  let report = json!({"kind": "delivery.report", "authority": module_authority, "source_commit": source,
    "base_commit": base, "outcome": {"kind": "delivered", "merge_commit": "e".repeat(40)}});
  let (status, delivered) = team.command(&report);
  assert_eq!(status, 201, "{delivered}");
}

#[test]
fn team_lifecycle_a_report_with_unreadable_artifacts_changes_nothing() {
  let team = Team::start(Standing::Member, vec![task(1, "in_progress", Some(YOU))]);
  let named = start(&team, 1);
  let bad = json!({"kind": "work.report", "authority": named, "run_id": RunId::generate(),
    "event_id": WorkEventId::generate(), "sequence": 1, "outcome": {"kind": "succeeded"},
    "artifacts": [{"kind": "report"}]});
  let (status, refused) = team.command(&bad);
  assert_eq!((status, refused["code"].as_str()), (422, Some("invalid")));
  let read = team.task(1);
  assert_eq!(read["work"]["state"], "running", "the work is where it was");
  assert_eq!(read["effective_claim"]["holder"]["display_name"], "Bohdan Koval");
  assert_eq!(
    team.patch(1, &json!({"workflow": "done"})).0,
    200,
    "no report was accepted, so no run stands in the way of closing"
  );
}

#[test]
#[should_panic(expected = "no workflow status")]
fn team_lifecycle_a_workflow_name_that_is_no_workflow_is_refused_when_set_up() {
  let team = Team::start(Standing::Member, vec![task(1, "todo", None)]);
  team.server.authority_tasks(vec![task(1, "finished", None)]);
}

#[test]
fn team_lifecycle_closing_a_task_in_a_release_invalidates_the_release() {
  let tasks = vec![task(1, "in_progress", Some(YOU)), task(2, "in_progress", Some(YOU))];
  let team = Team::start(Standing::Admin, tasks);
  team.server.authority_delivers(2, Deliverable::Report);
  let release = team.create_release();
  for number in [1, 2] {
    assert_eq!(team.patch(number, &json!({"release": release})).0, 200);
  }
  let invalidated = |before: usize| -> Vec<Value> { team.batches().into_iter().skip(before).flatten().collect() };
  let expect = |scopes: &[Value], number: u32| {
    for expected in [
      json!({"kind": "task", "task": task_id(number)}),
      json!({"kind": "release", "release": release}),
      json!({"kind": "release_list"}),
    ] {
      assert!(
        scopes.contains(&expected),
        "task {number}: {expected} is invalidated: {scopes:?}"
      );
    }
  };

  let before = team.batches().len();
  assert_eq!(team.patch(1, &json!({"workflow": "done"})).0, 200);
  expect(&invalidated(before), 1);

  let before = team.batches().len();
  run(&team, 2, &json!({"kind": "succeeded"}), &report_artifact());
  assert_eq!(workflow(&team.task(2)), "done");
  expect(&invalidated(before), 2);
}
