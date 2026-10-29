//! A tool request auto mode's reviewer is still judging waits for the reviewer, not for the person: it is no item of
//! Needs you and counts in none, its run reads as at work, and its card in the conversation does not wait for the
//! person. Once the review leaves it to the person (it held, asked, failed or ran out of time), it waits for them as
//! any request does.

mod support;

use orchester_core::{
  id::TaskId,
  resource::ModelId,
  run::{
    PendingToolApproval, TaskPhase,
    review::{Review, ReviewFailure},
  },
  store::ActivityStore,
};
use orchester_exec::programs::Programs;
use orchester_workspace::tasks::{
  data::ProjectTasks,
  needs_you::{InPlace, WaitingKind},
  timeline::{ItemKind, approval_key, waits_for_you},
};
use support::{Fixture, at};

/// A project whose one run at work asks to run a read-only command, which auto mode's reviewer is judging: the
/// fixture, the task and the request's id.
fn under_review() -> (Fixture, TaskId, PendingToolApproval) {
  let mut fixture = Fixture::new();
  let task = fixture.task("Show the changelog", at(10));
  let run = fixture.started_run(task, at(20));
  let execution = run.current_attempt().expect("attempt").executions()[0].id();
  let invocation = fixture
    .db
    .start_invocation(run.id(), execution, &ModelId::new("gpt-5.5").expect("model"), at(21))
    .expect("invocation")
    .id;
  let request = PendingToolApproval {
    id: format!("{invocation}:call_1"),
    run: run.id(),
    execution,
    invocation,
    agent: "Builder".to_owned(),
    command: true,
    scope: None,
    server: "command".to_owned(),
    action: "tail -n 3 CHANGELOG.md".to_owned(),
    target: None,
    requested_at: at(30),
    native: None,
  };
  fixture.db.request_tool_approval(&request).expect("the request waits");
  fixture
    .db
    .set_reviewing(run.id(), &request.id, true)
    .expect("under review");
  (fixture, task, request)
}

fn tasks(fixture: &Fixture) -> ProjectTasks {
  ProjectTasks::load(fixture.repo.path(), &Programs::processes()).expect("load the project's tasks")
}

/// Whether the card of `request` in the conversation of `task` waits for the person.
fn card_waits(fixture: &Fixture, task: TaskId, request: &PendingToolApproval) -> bool {
  let read = InPlace::read(fixture.repo.path(), task).expect("read the task");
  let card = read
    .card(&approval_key(&request.id), false)
    .expect("the request's card");
  assert!(matches!(card.kind, ItemKind::Approval(_)));
  waits_for_you(&card)
}

#[test]
fn auto_mode_request_under_review_is_not_in_needs_you_and_its_run_is_at_work() {
  let (fixture, task, request) = under_review();
  let tasks = tasks(&fixture);
  assert!(tasks.waiting().is_empty(), "nothing waits for the person");
  assert_eq!(tasks.needs_you(), 0, "no Needs you counts it");
  assert!(tasks.pending_tool_approvals.is_empty());
  let reviewing: Vec<_> = tasks
    .reviewing_tool_approvals
    .iter()
    .map(|shown| (shown.approval.id.clone(), shown.reviewing))
    .collect();
  assert_eq!(reviewing, [(request.id.clone(), true)]);
  assert_eq!(
    tasks.phases.get(&task).and_then(|latest| latest.phase()),
    Some(TaskPhase::Running),
    "the run reads as at work"
  );
  assert!(!card_waits(&fixture, task, &request), "its card waits for the reviewer");
}

#[test]
fn auto_mode_request_its_review_left_to_the_person_waits_for_them() {
  let held = Review::Failed {
    failure: ReviewFailure::TimedOut,
    model: Some("claude-haiku-4-5".to_owned()),
  };
  let (mut fixture, task, request) = under_review();
  fixture
    .db
    .record_review(request.run, &request.id, &held)
    .expect("the review ran out of time");
  let tasks = tasks(&fixture);
  let items = tasks.waiting();
  assert_eq!(items.len(), 1);
  let WaitingKind::Approval(shown) = &items[0].kind else {
    panic!("the request waits for the person");
  };
  assert!(!shown.reviewing);
  assert_eq!(shown.review.as_ref(), Some(&held));
  assert_eq!(tasks.needs_you(), 1);
  assert!(tasks.reviewing_tool_approvals.is_empty());
  assert_eq!(
    tasks.phases.get(&task).and_then(|latest| latest.phase()),
    Some(TaskPhase::Waiting)
  );
  assert!(card_waits(&fixture, task, &request));
}
