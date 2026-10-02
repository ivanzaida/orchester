//! Retries end to end against `orchester-fake-codex`: a failed verdict sends its report back to the
//! step it returns to, a turn that fails without a result restarts its step once, and a run with no
//! automatic route left waits for a person, whose decision the runner carries out.
//! Needs `git`. Run with `cargo test -p orchester-runner --features test-agent --test retries retries`.

mod support;

use orchester_core::{
  connection::stream::InvocationOutcome,
  id::{IdempotencyKey, RunId},
  resource::StepKey,
  run::{AttemptReason, ReviewNote, Run, RunFailure, RunStatus, StepFailure, Stuck, StuckDecision},
  store::ActivityStore,
};
use orchester_ipc::{Event, Rejection};
use orchester_store::ProjectDb;
use tokio::sync::mpsc;

use self::support::{Setup, StepSpec, project, reported, scheduler, wait_for, wait_for_idle};

fn key(value: &str) -> StepKey {
  StepKey::new(value).expect("key")
}

fn waits(run: &Run) -> bool {
  matches!(run.status(), RunStatus::WaitingForPerson(_))
}

async fn started(
  setup: &Setup,
) -> (
  support::TestProject,
  orchester_runner::execution::Scheduler,
  mpsc::UnboundedReceiver<Event>,
  RunId,
) {
  let project = project(setup);
  let (scheduler, events) = scheduler(project.repo.path());
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");
  (project, scheduler, events, id)
}

#[tokio::test(flavor = "multi_thread")]
async fn retries_a_failed_review_sends_its_findings_to_implement() {
  let setup = Setup {
    steps: vec![
      StepSpec::new("implement", "Implement", "Change README.md.", &[]),
      StepSpec::new("review", "Review", "Fail once. Leave a handoff.", &["implement"]).retry("implement", 2),
    ],
    ..Setup::new("Say hello.")
  };
  let (project, _scheduler, mut events, id) = started(&setup).await;
  let root = project.repo.path();
  let run = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  let db = ProjectDb::open(root).expect("open");
  let prompts = reported(&db, id, "prompt| ");
  // Implement's second prompt says why the work came back, with Review's whole result quoted.
  for line in [
    "# Returned from Review (`review`), attempt 1",
    "## What Review reported",
    "> Finished Review",
    "> The next agent should know:",
    "> - README.md ends with hello",
  ] {
    assert!(prompts.iter().any(|prompt| prompt == line), "{line} in {prompts:?}");
  }
  // Review, running again, reads its own earlier result to check it.
  assert!(
    prompts.iter().any(|line| line == "# Your result in attempt 1"),
    "{prompts:?}"
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn retries_a_turn_that_fails_without_a_result_restarts_its_step_once_then_waits() {
  let (project, scheduler, mut events, id) = started(&Setup::new("Please fail.")).await;
  let root = project.repo.path();
  let run = wait_for(root, &mut events, id, waits).await;
  wait_for_idle(&scheduler).await;
  assert_eq!(
    run.status(),
    &RunStatus::WaitingForPerson(Stuck::BrokeAgain(key("implement")))
  );
  assert_eq!(run.attempts().len(), 2);
  assert_eq!(
    run.attempts()[1].reason(),
    &AttemptReason::Restart { step: key("implement") }
  );
  let db = ProjectDb::open(root).expect("open");
  let invocations = db.invocations(id).expect("invocations");
  assert_eq!(invocations.len(), 2, "one fresh restart");
  assert!(invocations.iter().all(|invocation| {
    matches!(&invocation.outcome, Some(InvocationOutcome::TurnFailed { message }) if message.contains("not supported"))
  }));
  assert!(scheduler.active_runs().is_empty(), "a waiting run holds no agent slot");

  // One more retry restarts the step again, and it waits again.
  scheduler.decide_stuck(id, StuckDecision::Retry).await.expect("retried");
  let run = wait_for(root, &mut events, id, |run| waits(run) && run.attempts().len() == 3).await;
  wait_for_idle(&scheduler).await;
  assert_eq!(db.invocations(id).expect("invocations").len(), 3);

  scheduler.decide_stuck(id, StuckDecision::Stop).await.expect("stopped");
  let stopped = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(
    stopped.status(),
    &RunStatus::Failed(RunFailure::StepFailed(key("implement")))
  );
  assert_eq!(stopped.attempts().len(), run.attempts().len());
  assert_eq!(
    scheduler.decide_stuck(id, StuckDecision::Retry).await,
    Err(Rejection::RunNotWaiting)
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn retries_exhausted_wait_and_a_person_returns_the_work_with_a_note() {
  let setup = Setup {
    steps: vec![
      StepSpec::new("implement", "Implement", "Change README.md. Wait for release.", &[]),
      StepSpec::new("review", "Review", "Report failure.", &["implement"]).retry("implement", 1),
    ],
    ..Setup::new("Say hello.")
  };
  let project = project(&setup);
  let root = project.repo.path();
  let release = root.join(".orchester").join("fake-release");
  std::fs::write(&release, "").expect("Implement finishes at once");
  let (scheduler, mut events) = scheduler(root);
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");
  let run = wait_for(root, &mut events, id, waits).await;
  wait_for_idle(&scheduler).await;
  assert_eq!(
    run.status(),
    &RunStatus::WaitingForPerson(Stuck::RetriesExhausted(key("review")))
  );
  assert_eq!(run.attempts().len(), 2);
  assert_eq!(
    run
      .attempts()
      .iter()
      .flat_map(|attempt| attempt.executions())
      .filter_map(|execution| execution.failure())
      .collect::<Vec<_>>(),
    [StepFailure::ReportedFailed, StepFailure::ReportedFailed]
  );

  let note = ReviewNote::new("Keep the greeting on one line.").expect("note");
  let decision = StuckDecision::ReturnTo {
    step: key("implement"),
    note,
  };
  std::fs::remove_file(&release).expect("Implement waits now");
  scheduler.decide_stuck(id, decision).await.expect("returned");
  // While the returned attempt runs, the run waits for no decision.
  assert_eq!(
    scheduler.decide_stuck(id, StuckDecision::Retry).await,
    Err(Rejection::RunNotWaiting)
  );
  std::fs::write(&release, "").expect("Implement finishes");
  let again = wait_for(root, &mut events, id, |run| waits(run) && run.attempts().len() == 3).await;
  assert!(matches!(
    again.attempts()[2].reason(),
    AttemptReason::Returned { return_to, .. } if *return_to == key("implement")
  ));
  let db = ProjectDb::open(root).expect("open");
  let prompts = reported(&db, id, "prompt| ");
  assert!(
    prompts.iter().any(|line| line == "## The person's instruction"),
    "{prompts:?}"
  );
  assert!(
    prompts.iter().any(|line| line == "Keep the greeting on one line."),
    "{prompts:?}"
  );
  assert!(
    prompts
      .iter()
      .any(|line| line == "# Returned from Review (`review`), attempt 2"),
    "{prompts:?}"
  );
}
