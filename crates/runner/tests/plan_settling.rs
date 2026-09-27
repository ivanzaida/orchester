//! A plan decision that arrives once the run waits for it, but before the task that ran the gated
//! step has ended: that task no longer reads requests and drops the decision unread as it ends.
//! The decision is still recorded and a new task executes the run; it is never lost, and the
//! request is answered only then. The task is held at its last step through the runner's log, so
//! the moment does not depend on timing. Needs `git`. Run with
//! `cargo test -p orchester-runner --features test-agent --test plan_settling`.

mod support;

use std::{future::Future, task::Poll};

use orchester_core::{
  id::IdempotencyKey,
  run::{PlanDecision, RunStatus},
  store::RunStore,
};
use orchester_store::ProjectDb;

use self::support::{PATIENCE, Setup, StepSpec, hold::hold_first_settled, project, scheduler, wait_for};

#[tokio::test(flavor = "multi_thread")]
async fn plan_settling_a_decision_the_ending_task_drops_still_continues_the_run() {
  let hold = hold_first_settled();
  let project = project(&Setup {
    steps: vec![
      StepSpec::new("plan", "Plan", "Propose a plan.", &[]).gate(),
      StepSpec::new("implement", "Implement", "Follow the plan.", &["plan"]),
    ],
    ..Setup::new("Say hello.")
  });
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .expect("the run starts");
  tokio::time::timeout(PATIENCE, hold.held)
    .await
    .expect("the plan waits in time")
    .expect("the task is held");
  let db = ProjectDb::open(root).expect("open");
  assert!(db.run(id).expect("read").expect("run").waits_for_a_person());
  let plan = db.plans(id).expect("plans").pop().expect("a plan");

  let decide = scheduler.decide_plan(id, plan.execution, plan.version, PlanDecision::Approved);
  tokio::pin!(decide);
  let first = std::future::poll_fn(|cx| Poll::Ready(decide.as_mut().poll(cx))).await;
  assert!(first.is_pending(), "the decision waits for the task: {first:?}");
  hold.release.send(()).expect("the task is held");
  let decided = tokio::time::timeout(PATIENCE, decide).await.expect("answered in time");
  assert_eq!(decided, Ok(()));
  let done = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(done.status(), &RunStatus::Succeeded);
  assert_eq!(
    db.plans(id).expect("plans")[0].decision(),
    Some(&PlanDecision::Approved)
  );
}
