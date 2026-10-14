//! A step that reports `unverified` end to end against `orchester-fake-codex`: the run waits for the person without
//! taking the step's failure route or using a retry, and the person accepts the step, runs it again or returns the
//! work; a step is not run again on the revision its latest failed or unverified result was for.
//! Needs `git`. Run with `cargo test -p orchester-runner --features test-agent --test unverified unverified`.

mod support;

use orchester_core::{
  id::{IdempotencyKey, RunId},
  resource::StepKey,
  run::{AttemptReason, ReviewNote, Run, RunStatus, StepFailure, StepOutcome, Stuck, StuckDecision},
  store::ActivityStore,
};
use orchester_ipc::Event;
use orchester_store::ProjectDb;
use tokio::sync::mpsc;

use self::support::{Setup, StepSpec, project, scheduler, wait_for, wait_for_idle};

fn key(value: &str) -> StepKey {
  StepKey::new(value).expect("key")
}

fn waits(run: &Run) -> bool {
  matches!(run.status(), RunStatus::WaitingForPerson(_))
}

struct Started {
  project: support::TestProject,
  scheduler: orchester_runner::execution::Scheduler,
  events: mpsc::UnboundedReceiver<Event>,
  id: RunId,
}

impl Started {
  async fn new(steps: Vec<StepSpec>) -> Self {
    let project = project(&Setup {
      steps,
      ..Setup::new("Say hello.")
    });
    let (scheduler, events) = scheduler(project.repo.path());
    let id = scheduler
      .start_run(IdempotencyKey::generate(), project.task)
      .await
      .expect("the run starts");
    Self {
      project,
      scheduler,
      events,
      id,
    }
  }

  async fn until(&mut self, done: impl Fn(&Run) -> bool) -> Run {
    let run = wait_for(self.project.repo.path(), &mut self.events, self.id, done).await;
    wait_for_idle(&self.scheduler).await;
    run
  }

  fn db(&self) -> ProjectDb {
    ProjectDb::open(self.project.repo.path()).expect("open")
  }

  fn invocations(&self) -> usize {
    self.db().invocations(self.id).expect("invocations").len()
  }
}

/// Implement, then Verify, which returns to Implement at most twice. A Verify that changes nothing leaves the run's
/// revision as Implement left it, as a read-only Verify does.
fn verify_after(implement: &'static str, verify: &'static str) -> Vec<StepSpec> {
  vec![
    StepSpec::new("implement", "Implement", implement, &[]),
    StepSpec::new("verify", "Verify", verify, &["implement"]).retry("implement", 2),
  ]
}

#[tokio::test(flavor = "multi_thread")]
async fn unverified_waits_for_the_person_without_a_retry() {
  let mut started = Started::new(verify_after("Change README.md.", "Report unverified.")).await;
  let run = started.until(waits).await;
  assert_eq!(
    run.status(),
    &RunStatus::WaitingForPerson(Stuck::Unverified(key("verify")))
  );
  assert_eq!(run.attempts().len(), 1, "no failure route was taken");
  assert_eq!(run.retries_taken(&key("verify")), 0);
  assert_eq!(started.invocations(), 2, "Implement and Verify once each");
  let failed = run.latest_execution(&key("verify")).expect("verify");
  assert_eq!(failed.failure(), Some(StepFailure::Unverified));
  assert!(failed.judged().is_some(), "the revision it was for is kept");
  let outputs = started.db().step_outputs(started.id).expect("outputs");
  let output = outputs
    .iter()
    .find(|output| output.step == key("verify"))
    .expect("verify's output");
  assert_eq!(output.outcome, Some(StepOutcome::Unverified));
  assert_eq!(output.unverified[0].check, "Summary 02.B");
  assert_eq!(output.unverified[0].reason, "no fixture reaches it");
  assert!(
    started.scheduler.active_runs().is_empty(),
    "a waiting run holds no agent slot"
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn unverified_accept_passes_the_step_and_runs_the_rest() {
  let mut steps = verify_after("Change README.md.", "Report unverified.");
  steps.push(StepSpec::new("publish", "Publish", "Say hello.", &["verify"]));
  let mut started = Started::new(steps).await;
  started.until(waits).await;
  started
    .scheduler
    .decide_stuck(started.id, StuckDecision::Accept)
    .await
    .expect("accepted");
  let run = started.until(|run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  assert_eq!(
    run.attempts()[1].reason(),
    &AttemptReason::Accepted { step: key("verify") }
  );
  assert_eq!(started.invocations(), 3, "Verify does not run again; Publish runs");
}

#[tokio::test(flavor = "multi_thread")]
async fn unverified_accept_of_the_last_step_succeeds_the_run() {
  let mut started = Started::new(verify_after("Change README.md.", "Report unverified.")).await;
  started.until(waits).await;
  started
    .scheduler
    .decide_stuck(started.id, StuckDecision::Accept)
    .await
    .expect("accepted");
  let run = started.until(|run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  assert_eq!(run.attempts().len(), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn unverified_retry_runs_the_step_again_on_the_same_revision() {
  let mut started = Started::new(verify_after("Change README.md.", "Report unverified.")).await;
  started.until(waits).await;
  started
    .scheduler
    .decide_stuck(started.id, StuckDecision::Retry)
    .await
    .expect("retried");
  let run = started.until(|run| waits(run) && run.attempts().len() == 2).await;
  assert_eq!(
    run.attempts()[1].reason(),
    &AttemptReason::Restart { step: key("verify") }
  );
  assert_eq!(started.invocations(), 3, "the person asked: Verify ran again");
  assert_eq!(
    run.status(),
    &RunStatus::WaitingForPerson(Stuck::Unverified(key("verify")))
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn unverified_returned_work_that_changed_nothing_is_not_verified_again() {
  let mut started = Started::new(verify_after("Change nothing.", "Report unverified. Change nothing.")).await;
  started.until(waits).await;
  let decision = StuckDecision::ReturnTo {
    step: key("implement"),
    note: ReviewNote::new("Add the fixture for Summary 02.B.").expect("note"),
  };
  started
    .scheduler
    .decide_stuck(started.id, decision)
    .await
    .expect("returned");
  let run = started.until(|run| waits(run) && run.attempts().len() == 2).await;
  assert_eq!(
    run.status(),
    &RunStatus::WaitingForPerson(Stuck::Unchanged(key("verify")))
  );
  assert_eq!(started.invocations(), 3, "Implement ran again; Verify did not");
  // The person may still accept it as it is.
  started
    .scheduler
    .decide_stuck(started.id, StuckDecision::Accept)
    .await
    .expect("accepted");
  let run = started.until(|run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
}

#[tokio::test(flavor = "multi_thread")]
async fn unverified_a_failed_verdict_on_an_unchanged_revision_is_not_run_again() {
  let mut started = Started::new(verify_after("Change nothing.", "Report failure. Change nothing.")).await;
  let run = started.until(waits).await;
  assert_eq!(
    run.status(),
    &RunStatus::WaitingForPerson(Stuck::Unchanged(key("verify")))
  );
  assert_eq!(run.retries_taken(&key("verify")), 1, "one retry went back to Implement");
  assert_eq!(started.invocations(), 3, "Implement twice, Verify once");
}
