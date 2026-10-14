//! A QA app and its script never outlive what started them: the runner failing while the app runs stops it and frees
//! the slot, and a run cancelled while `tools/qa-app.ps1` still runs ends the script with what it started.
//! Windows only, as `qa_app`: the stand-in script (`support::qa`) is Windows PowerShell.
//! Needs `git`. Run with `cargo test -p orchester-runner --features test-agent --test qa_app_stop qa_app`.
#![cfg(windows)]

mod support;

use std::fs;

use orchester_core::{
  id::IdempotencyKey,
  run::{InterruptCause, RunStatus},
};

use self::support::{
  StepSpec,
  qa::{approve, gone, qa_project, reported_pid, started_pid, verify_step},
  scheduler, wait_for, wait_for_idle,
};

/// Makes the project database refuse every invocation's end, as a broken or full database would.
const REFUSE_ENDS: &str = "CREATE TRIGGER refuse_end BEFORE UPDATE ON invocations \
  WHEN NEW.ended_at IS NOT NULL BEGIN SELECT RAISE(FAIL, 'worker end refused by the test'); END;";

#[tokio::test(flavor = "multi_thread")]
async fn qa_app_stops_when_the_runner_fails_while_it_runs() {
  let project = qa_project(vec![
    StepSpec::new("implement", "Implement", "Wait for release.", &[]),
    StepSpec::new("verify", "Verify", "Start the QA app. Hold the QA app.", &[]),
  ]);
  let root = project.repo.path();
  let orchester = root.join(".orchester");
  fs::write(orchester.join("fake-hold"), "hold").expect("hold");
  let (scheduler, mut events) = scheduler(root);
  scheduler
    .set_agent_slots(std::num::NonZeroU32::new(2).expect("two"))
    .await
    .expect("two agents at once");
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("run");
  approve(&scheduler, root, &mut events, id).await;
  let app = started_pid(root, &mut events, id).await;
  // Implement's end cannot be recorded: the runner fails and ends every agent, Verify's with its QA app.
  rusqlite::Connection::open(orchester.join("orchester.db"))
    .expect("open fixture")
    .execute_batch(REFUSE_ENDS)
    .expect("refuse ends");
  fs::write(orchester.join("fake-release"), "go").expect("release");
  let run = wait_for(root, &mut events, id, |run| run.status() != &RunStatus::Running).await;
  wait_for_idle(&scheduler).await;
  assert_eq!(run.status(), &RunStatus::Interrupted(InterruptCause::RunnerFailed));
  gone(&app, "the QA app after the runner failed").await;
}

#[tokio::test(flavor = "multi_thread")]
async fn qa_app_a_cancelled_run_ends_the_script_and_what_it_started() {
  let project = qa_project(verify_step("Start the QA app. QA scenario hang."));
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("run");
  approve(&scheduler, root, &mut events, id).await;
  let script = reported_pid(root, "run-1-verify", "script").await;
  let app = reported_pid(root, "run-1-verify", "app").await;
  scheduler.cancel_run(id).await.expect("cancelled");
  let run = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Cancelled);
  gone(&script, "the QA script after its run was cancelled").await;
  gone(&app, "the app the cancelled script started").await;
}
