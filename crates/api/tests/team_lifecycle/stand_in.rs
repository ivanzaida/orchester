//! A team task's close, reopen and block on the stand-in team server, as the contract orders them: by the caller's
//! claim and role on the task patch, with the claim released on a close.

use orchester_api::{
  test_support::team_server::Standing,
  v2::{DenialReason, LifecycleAction},
};
use serde_json::{Value, json};

use crate::team::{OLHA, Team, YOU, lifecycle, report, report_artifact, start, task, workflow};

/// A task its caller holds, one somebody else holds, a Done one and a Backlog one.
fn capability_tasks() -> Vec<orchester_api::test_support::team_server::AuthorityTask> {
  vec![
    task(1, "todo", Some(YOU)),
    task(2, "todo", Some(OLHA)),
    task(3, "done", None),
    task(4, "backlog", None),
  ]
}

#[test]
fn team_lifecycle_capabilities_follow_the_claim_and_the_role() {
  use DenialReason::{ClaimRequired, RoleRequired, TransitionDenied};
  let member = Team::start(Standing::Member, capability_tasks());
  let expect = |number, expected: [Option<DenialReason>; 5]| {
    let read = lifecycle(&member.task(number));
    assert_eq!(
      read.iter().map(|(_, reason)| *reason).collect::<Vec<_>>(),
      expected,
      "task {number}"
    );
    assert_eq!(
      read.iter().map(|(action, _)| *action).collect::<Vec<_>>(),
      LifecycleAction::ALL
    );
  };
  // Order: Complete, Cancel, Reopen, Triage, Block.
  expect(
    1,
    [
      None,
      Some(RoleRequired),
      Some(TransitionDenied),
      Some(RoleRequired),
      None,
    ],
  );
  expect(
    2,
    [
      Some(ClaimRequired),
      Some(RoleRequired),
      Some(TransitionDenied),
      Some(RoleRequired),
      Some(ClaimRequired),
    ],
  );
  expect(
    3,
    [
      Some(TransitionDenied),
      Some(TransitionDenied),
      Some(RoleRequired),
      Some(TransitionDenied),
      Some(ClaimRequired),
    ],
  );
  expect(
    4,
    [
      Some(ClaimRequired),
      Some(RoleRequired),
      Some(TransitionDenied),
      Some(RoleRequired),
      Some(ClaimRequired),
    ],
  );
}

#[test]
fn team_lifecycle_an_admin_has_every_action_the_task_has() {
  use DenialReason::TransitionDenied;
  use LifecycleAction::{Block, Cancel, Complete, Reopen, Triage};
  for standing in [Standing::Admin, Standing::ServerAdmin] {
    let admin = Team::start(standing, capability_tasks());
    let allowed = |number, action: LifecycleAction| {
      lifecycle(&admin.task(number))
        .into_iter()
        .find(|(known, _)| *known == action)
        .expect("listed")
        .1
    };
    assert_eq!(
      allowed(2, Complete),
      None,
      "{standing:?} closes a task somebody else holds"
    );
    assert_eq!(allowed(2, Cancel), None);
    assert_eq!(allowed(2, Block), None);
    assert_eq!(allowed(3, Reopen), None);
    assert_eq!(allowed(4, Triage), None);
    assert_eq!(
      allowed(3, Complete),
      Some(TransitionDenied),
      "the state is told before the caller"
    );
    assert_eq!(allowed(2, Reopen), Some(TransitionDenied));
  }
}

#[test]
fn team_lifecycle_the_holder_marks_done_and_the_claim_is_released() {
  let team = Team::start(
    Standing::Member,
    vec![task(1, "todo", Some(YOU)), task(2, "in_progress", Some(YOU))],
  );
  let before = team.task(1);
  assert_eq!(before["effective_claim"]["holder"]["display_name"], "Bohdan Koval");
  let (status, answer) = team.patch(1, &json!({"workflow": "done"}));
  assert_eq!(status, 200, "{answer}");
  assert_eq!(
    (workflow(&answer["task"]), &answer["task"]["revision"]),
    ("done", &json!(2))
  );

  let after = team.task(1);
  assert_eq!(
    (workflow(&after), &after["effective_claim"], &after["work"]),
    ("done", &Value::Null, &Value::Null)
  );
  assert_eq!(after["updated"]["by"]["display_name"], "Bohdan Koval");
  let denied = Some(DenialReason::TransitionDenied);
  let offers: Vec<_> = lifecycle(&after).into_iter().map(|(_, reason)| reason).collect();
  let (complete, cancel, reopen, triage, block) = (
    denied,
    denied,
    Some(DenialReason::RoleRequired),
    denied,
    Some(DenialReason::ClaimRequired),
  );
  assert_eq!(
    offers,
    [complete, cancel, reopen, triage, block],
    "a closed task offers a holder nothing"
  );
  let (status, again) = team.patch_on(&before, 1, &json!({"workflow": "done"}));
  assert_eq!(
    (status, again["error"].as_str(), workflow(&again["current"])),
    (409, Some("revision_conflict"), "done"),
    "the holder's retry of the applied change reads the task as it is, not a refusal of the role"
  );
  let (status, again) = team.patch(1, &json!({"workflow": "done"}));
  assert_eq!(
    (status, again["error"].as_str()),
    (403, Some("not_admin")),
    "the empty change on the current revision is an Admin's, as it was"
  );
  assert_eq!(workflow(&team.task(2)), "in_progress", "another task is untouched");
}

#[test]
fn team_lifecycle_refusals_name_what_the_caller_lacks() {
  let team = Team::start(
    Standing::Member,
    vec![
      task(1, "todo", Some(YOU)),
      task(2, "todo", Some(OLHA)),
      task(3, "done", None),
      task(4, "backlog", None),
    ],
  );
  let (status, refused) = team.patch(2, &json!({"workflow": "done"}));
  assert_eq!(
    (status, refused["error"].as_str()),
    (409, Some("not_holder")),
    "{refused}"
  );
  assert!(
    refused.get("claim").is_some(),
    "the refusal carries the task's claim, null or not"
  );
  let (status, refused) = team.patch(4, &json!({"blocked_reason": "waiting"}));
  assert_eq!(
    (status, refused["error"].as_str()),
    (409, Some("not_holder")),
    "a task nobody holds"
  );

  for admins in [
    (1, json!({"workflow": "canceled"})),
    (3, json!({"workflow": "todo"})),
    (4, json!({"workflow": "todo"})),
    (1, json!({"workflow": "done", "title": "Renamed"})),
    (1, json!({"workflow": "in_progress"})),
    (2, json!({"workflow": "canceled"})),
  ] {
    let (status, refused) = team.patch(admins.0, &admins.1);
    assert_eq!(
      (status, refused["error"].as_str()),
      (403, Some("not_admin")),
      "{}",
      admins.1
    );
  }
  for number in [1, 2, 3, 4] {
    let task = team.task(number);
    assert_eq!(task["revision"], 1, "a refused patch changes nothing");
  }
}

#[test]
fn team_lifecycle_an_admin_closes_reopens_and_moves_any_task() {
  let team = Team::start(
    Standing::Admin,
    vec![
      task(1, "todo", Some(OLHA)),
      task(2, "todo", Some(YOU)),
      task(3, "done", None),
      task(4, "backlog", None),
    ],
  );
  let (status, canceled) = team.patch(1, &json!({"workflow": "canceled"}));
  assert_eq!(status, 200, "{canceled}");
  let task = team.task(1);
  assert_eq!(
    (workflow(&task), &task["effective_claim"]),
    ("canceled", &Value::Null),
    "the holder's claim is released"
  );

  let (status, done) = team.patch(2, &json!({"workflow": "done"}));
  assert_eq!((status, workflow(&done["task"])), (200, "done"));
  assert_eq!(team.task(2)["effective_claim"], Value::Null);

  assert_eq!(
    workflow(&team.patch(3, &json!({"workflow": "backlog"})).1["task"]),
    "backlog",
    "reopened"
  );
  assert_eq!(
    workflow(&team.patch(3, &json!({"workflow": "todo"})).1["task"]),
    "todo",
    "Backlog to Todo"
  );
  assert_eq!(workflow(&team.patch(4, &json!({"workflow": "todo"})).1["task"]), "todo");
  assert_eq!(
    workflow(&team.patch(1, &json!({"workflow": "todo"})).1["task"]),
    "todo",
    "a canceled task reopens free"
  );
  assert_eq!(team.task(1)["effective_claim"], Value::Null);
}

#[test]
fn team_lifecycle_the_manual_blocked_is_the_holders_or_an_admins() {
  let team = Team::start(
    Standing::Member,
    vec![task(1, "todo", Some(YOU)), task(2, "todo", Some(OLHA))],
  );
  let (status, blocked) = team.patch(1, &json!({"blocked_reason": "waiting for the API key"}));
  assert_eq!(
    (status, blocked["task"]["blocked_reason"].as_str()),
    (200, Some("waiting for the API key"))
  );
  assert_eq!(
    team.task(1)["effective_claim"]["holder"]["display_name"],
    "Bohdan Koval",
    "blocking keeps the claim"
  );
  let (status, cleared) = team.patch(1, &json!({"blocked_reason": null}));
  assert_eq!((status, &cleared["task"]["blocked_reason"]), (200, &Value::Null));
  let (status, empty) = team.patch(1, &json!({"blocked_reason": "  "}));
  assert_eq!(
    (status, empty["error"].as_str(), empty["reason"].as_str()),
    (422, Some("organization"), Some("missing_reason"))
  );
  let (status, refused) = team.patch(2, &json!({"blocked_reason": "waiting"}));
  assert_eq!((status, refused["error"].as_str()), (409, Some("not_holder")));

  let admin = Team::start(Standing::Admin, vec![task(2, "todo", Some(OLHA))]);
  assert_eq!(
    admin.patch(2, &json!({"blocked_reason": "waiting"})).0,
    200,
    "an Admin blocks another's task"
  );
}

#[test]
fn team_lifecycle_a_stale_revision_is_the_usual_conflict() {
  let team = Team::start(Standing::Member, vec![task(1, "todo", Some(YOU))]);
  let task = team.task(1);
  let (status, conflict) = team.patch_on(&task, 7, &json!({"workflow": "done"}));
  assert_eq!((status, conflict["error"].as_str()), (409, Some("revision_conflict")));
  assert_eq!(
    conflict["current"]["revision"], 1,
    "the refusal carries the task as it is"
  );
  assert_eq!(workflow(&team.task(1)), "todo");
}

#[test]
fn team_lifecycle_a_task_whose_run_has_not_ended_is_closed_after_the_run_stops() {
  let team = Team::start(Standing::Member, vec![task(1, "in_progress", Some(YOU))]);
  let named = start(&team, 1);
  report(&team, &named, 1, (&json!({"kind": "progress"}), &json!([])));
  let (status, refused) = team.patch(1, &json!({"workflow": "done"}));
  assert_eq!(
    (status, refused["error"].as_str(), refused["reason"].as_str()),
    (422, Some("organization"), Some("stop_run_first"))
  );
  assert_eq!(workflow(&team.task(1)), "in_progress");

  report(&team, &named, 2, (&json!({"kind": "succeeded"}), &report_artifact()));
  let (status, done) = team.patch(1, &json!({"workflow": "done"}));
  assert_eq!(
    (status, workflow(&done["task"])),
    (200, "done"),
    "the run ended: the holder closes the task"
  );
}
