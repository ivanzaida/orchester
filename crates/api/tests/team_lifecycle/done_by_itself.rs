//! A task becomes Done by itself on the stand-in team server: when the server accepts the delivery of its work, or
//! when the run of report and outside work succeeds.

use orchester_api::test_support::team_server::{AuthorityTask, Standing};
use orchester_core::resource::Deliverable;
use serde_json::{Value, json};

use crate::team::{Team, YOU, claim_id, module, patch_artifact, report_artifact, run, task, task_id, workflow};

#[test]
fn team_lifecycle_a_successful_run_alone_marks_only_work_without_a_base_merge() {
  let tasks = (1..=5).map(|number| task(number, "in_progress", Some(YOU))).collect();
  let team = Team::start(Standing::Member, tasks);
  team.server.authority_delivers(2, Deliverable::Report);
  team.server.authority_delivers(3, Deliverable::Outside);
  team.server.authority_delivers(4, Deliverable::ChangesOrReport);
  team.server.authority_delivers(5, Deliverable::ChangesOrReport);
  let success = json!({"kind": "succeeded"});
  run(&team, 1, &success, &report_artifact());
  run(&team, 2, &success, &report_artifact());
  run(&team, 3, &success, &json!([]));
  run(&team, 4, &success, &report_artifact());
  run(&team, 5, &success, &patch_artifact());

  let states: Vec<String> = (1..=5).map(|number| workflow(&team.task(number)).to_owned()).collect();
  assert_eq!(
    states,
    ["in_progress", "done", "done", "done", "in_progress"],
    "changes wait for their delivery"
  );
  let done = team.task(2);
  assert_eq!(
    (&done["effective_claim"], &done["revision"]),
    (&Value::Null, &json!(2)),
    "its claim is released"
  );
  assert_eq!(
    team.task(1)["effective_claim"]["holder"]["display_name"],
    "Bohdan Koval",
    "the changes' claim is kept"
  );
}

#[test]
fn team_lifecycle_an_accepted_delivery_marks_the_modules_succeeded_work_done() {
  let mut tasks: Vec<AuthorityTask> = (1..=4).map(|number| task(number, "in_progress", Some(YOU))).collect();
  tasks[1].work = Some("running");
  tasks[2].workflow = "canceled";
  for task in &mut tasks {
    task.module = Some(0);
  }
  let team = Team::start(Standing::Member, tasks);
  team.server.authority_modules(vec![module(Some(YOU))]);
  team.server.automatic_merge(true);
  for number in [1, 3, 4] {
    run(&team, number, &json!({"kind": "succeeded"}), &patch_artifact());
  }
  assert_eq!(
    (1..=4)
      .map(|number| workflow(&team.task(number)).to_owned())
      .collect::<Vec<_>>(),
    ["in_progress", "in_progress", "canceled", "in_progress"],
    "a successful run delivers nothing to base"
  );

  let module_authority = json!({"subject": {"kind": "module", "id": "0191c7c0-0000-7000-8000-000000200000"},
    "claim_id": claim_id(0, 0x800), "work_id": null});
  let (source, base) = ("c".repeat(40), "d".repeat(40));
  let request = json!({"kind": "delivery.request", "authority": module_authority, "authorization_operation_id": null,
    "branch": "module/payments", "source_commit": source, "base_commit": base});
  let (status, requested) = team.command(&request);
  assert_eq!(status, 201, "{requested}");
  assert_eq!(
    workflow(&team.task(1)),
    "in_progress",
    "a requested delivery marks nothing"
  );

  let report = |outcome: Value| {
    team.command(
      &json!({"kind": "delivery.report", "authority": module_authority, "source_commit": source,
      "base_commit": base, "outcome": outcome}),
    )
  };
  assert_eq!(report(json!({"kind": "failed"})).0, 201);
  assert_eq!(
    workflow(&team.task(1)),
    "in_progress",
    "a failed delivery marks nothing"
  );
  assert_eq!(team.command(&request).0, 201);
  let (status, delivered) = report(json!({"kind": "delivered", "merge_commit": "e".repeat(40)}));
  assert_eq!(status, 201, "{delivered}");

  let states: Vec<String> = (1..=4).map(|number| workflow(&team.task(number)).to_owned()).collect();
  assert_eq!(
    states,
    ["done", "in_progress", "canceled", "done"],
    "the succeeded work of open tasks is delivered"
  );
  assert_eq!(
    team.task(1)["effective_claim"]["subject"]["kind"],
    "module",
    "the task's claim is released, the module's is not"
  );
  assert_eq!(
    team.task(2)["effective_claim"]["subject"]["kind"],
    "task",
    "the work that did not succeed is kept"
  );

  let module_id = "0191c7c0-0000-7000-8000-000000200000";
  let scopes: Vec<Value> = team.batches().into_iter().flatten().collect();
  for expected in [
    json!({"kind": "task", "task": task_id(1)}),
    json!({"kind": "module", "module": module_id}),
    json!({"kind": "module_list"}),
  ] {
    assert!(scopes.contains(&expected), "{expected} is invalidated");
  }
}
