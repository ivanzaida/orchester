//! The plan review (02.11 `x3lVC`) as data: the plan that waits, the earlier versions of its step,
//! who proposed them, and when there is nothing to review.

mod support;

use orchester_core::run::{PlanDecision, ReviewNote, RunStatus, TaskPhase};
use orchester_desktop::tasks::{
  detail::TaskDetail,
  plan::{PlanReview, state_key},
};

use self::support::{Fixture, at};

fn read(fixture: &Fixture, task: orchester_core::id::TaskId) -> (TaskDetail, Option<PlanReview>) {
  let detail = TaskDetail::load_in(fixture.repo.path(), task).expect("the task is read");
  let review = detail.run.as_ref().and_then(PlanReview::of);
  (detail, review)
}

#[test]
fn plan_review_reads_the_waiting_plan_and_its_earlier_versions() {
  let mut fixture = Fixture::gated();
  let task = fixture.task("Add login", at(10));
  let mut run = fixture.started_run(task, at(20));
  let mut first = fixture.propose(&mut run, "1. Replace sign-in.", at(30));
  let (detail, review) = read(&fixture, task);
  assert_eq!(detail.phase(), TaskPhase::Waiting);
  let review = review.expect("plan v1 waits");
  assert_eq!((&*review.step, &*review.agent), ("Implement", "Builder"));
  assert!(review.next.is_empty());
  assert_eq!(review.waiting().text, "1. Replace sign-in.");
  assert_eq!(state_key(review.waiting()), "runs.plan.version.awaiting");

  let note = ReviewNote::new("Keep email sign-in.").expect("a note");
  fixture.decide(&mut run, &mut first, PlanDecision::ChangesRequested { note }, at(40));
  // While the agent revises it there is nothing to decide.
  let (detail, review) = read(&fixture, task);
  assert_eq!((detail.phase(), review), (TaskPhase::Running, None));

  let mut second = fixture.propose(&mut run, "1. Add GitHub beside email.", at(50));
  let review = read(&fixture, task).1.expect("plan v2 waits");
  assert_eq!(review.plans().len(), 2);
  assert_eq!(review.waiting().version, second.version);
  let earlier = review.shown(Some(first.version));
  assert_eq!(earlier.text, "1. Replace sign-in.");
  assert_eq!(state_key(earlier), "runs.plan.version.changesRequested");
  assert_eq!(review.shown(None).version, second.version);

  fixture.decide(&mut run, &mut second, PlanDecision::Approved, at(60));
  let (detail, review) = read(&fixture, task);
  assert_eq!(review, None);
  let run = detail.run.expect("run");
  assert_eq!(run.run.status(), &RunStatus::Succeeded);
  assert_eq!(state_key(&run.plans[1]), "runs.plan.version.approved");
}
