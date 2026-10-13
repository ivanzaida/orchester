//! A run whose task fails on the runner's own side: the project database refuses a write the step's end
//! needs. The run does not stay running with nothing behind it: it is interrupted for the runner's
//! failure, from what was stored, and the error is in its activity. Needs `git`. Run with `cargo test -p
//! orchester-runner --features test-agent --test store_refusal store_refusal`.

mod support;

use std::num::NonZeroU32;

use orchester_core::{
  connection::stream::{AgentEvent, InvocationOutcome, RunnerProblem},
  id::IdempotencyKey,
  run::{InterruptCause, RunStatus, active_time},
  store::{ActivityStore, RunStore},
  time::Timestamp,
};
use orchester_store::ProjectDb;

use self::support::{
  Setup, StepSpec, project, queue_another_task, scheduler, tools::ToolSpec, wait_for, wait_for_idle,
};

/// Makes the project database refuse every step output, as a broken or full database would.
const REFUSE_OUTPUTS: &str = "CREATE TRIGGER refuse_outputs BEFORE INSERT ON step_outputs \
                              BEGIN SELECT RAISE(ABORT, 'refused by the test'); END";

/// FAIL keeps the trigger's counter change, so only the worker's first end write fails.
/// Cleanup can then repair it without the test racing the worker to remove the trigger.
const REFUSE_END_ONCE: &str = "CREATE TABLE fault (remaining INTEGER); INSERT INTO fault VALUES (1); \
  CREATE TRIGGER refuse_end BEFORE UPDATE ON invocations \
  WHEN NEW.ended_at IS NOT NULL AND (SELECT remaining FROM fault) > 0 \
  BEGIN UPDATE fault SET remaining = remaining - 1; \
  SELECT RAISE(FAIL, 'worker end refused by the test'); END;";

#[tokio::test(flavor = "multi_thread")]
async fn store_refusal_worker_end_retires_agent_and_preserves_original_recovery() {
  let project = project(&Setup::new("Append one line."));
  let root = project.repo.path();
  let raw = rusqlite::Connection::open(root.join(".orchester/orchester.db")).expect("open fixture");
  raw.execute_batch(REFUSE_END_ONCE).expect("inject failure");
  let (scheduler, mut events) = scheduler(root);
  scheduler.set_agent_slots(NonZeroU32::MIN).await.expect("one slot");
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("start");
  let interrupted = wait_for(root, &mut events, id, |run| run.status() != &RunStatus::Running).await;
  wait_for_idle(&scheduler).await;
  assert_eq!(
    interrupted.status(),
    &RunStatus::Interrupted(InterruptCause::RunnerFailed)
  );
  assert_failure_recorded(root, id);
  let db = ProjectDb::open(root).expect("open fixture");
  let calls = db.invocations(id).expect("invocations");
  assert_eq!(calls.len(), 1);
  assert!(calls[0].ended_at.is_some());
  assert_eq!(calls[0].outcome, Some(InvocationOutcome::Unrecorded));
  assert!(!calls[0].outcome.as_ref().expect("unknown end").is_success());
  let session = calls[0].session.clone().expect("durable session");
  let (continued, refused) = scheduler.continue_runs(&[id]).await;
  assert_eq!(continued, [id]);
  assert!(refused.is_empty(), "ordinary continuation: {refused:?}");
  let recovered = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  wait_for_idle(&scheduler).await;
  assert_eq!(recovered.status(), &RunStatus::Succeeded);
  assert_eq!(recovered.id(), id);
  assert_eq!(recovered.branch(), interrupted.branch());
  let calls = db.invocations(id).expect("resumed invocations");
  assert_eq!(calls.len(), 2);
  assert_eq!(calls[1].session.as_deref(), Some(session.as_str()));
  assert_eq!(db.runs_of_task(project.task).expect("runs").len(), 1);
  let second = scheduler
    .start_run(IdempotencyKey::generate(), queue_another_task(root))
    .await
    .expect("slot reused");
  assert_eq!(
    wait_for(root, &mut events, second, |run| run.status().is_terminal())
      .await
      .status(),
    &RunStatus::Succeeded
  );
  wait_for_idle(&scheduler).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn store_refusal_command_end_retires_both_command_slots() {
  let project = project(&Setup {
    steps: vec![StepSpec::new("check", "Check", "", &[]).command(ToolSpec::test_command(&["print", "1"]))],
    ..Setup::new("Run a harmless command.")
  });
  let root = project.repo.path();
  let raw = rusqlite::Connection::open(root.join(".orchester/orchester.db")).expect("open fixture");
  raw.execute_batch(REFUSE_END_ONCE).expect("inject failure");
  let (scheduler, mut events) = scheduler(root);
  for task in [project.task, queue_another_task(root)] {
    raw.execute("UPDATE fault SET remaining = 1", []).expect("arm fault");
    let id = scheduler
      .start_run(IdempotencyKey::generate(), task)
      .await
      .expect("start");
    let ended = wait_for(root, &mut events, id, |run| run.status() != &RunStatus::Running).await;
    wait_for_idle(&scheduler).await;
    assert_eq!(ended.status(), &RunStatus::Interrupted(InterruptCause::RunnerFailed));
    assert_failure_recorded(root, id);
    assert!(
      ProjectDb::open(root).expect("db").invocations(id).expect("invocations")[0]
        .ended_at
        .is_some()
    );
  }
  let healthy = scheduler
    .start_run(IdempotencyKey::generate(), queue_another_task(root))
    .await
    .expect("start healthy run");
  assert_eq!(
    wait_for(root, &mut events, healthy, |run| run.status().is_terminal())
      .await
      .status(),
    &RunStatus::Succeeded
  );
  wait_for_idle(&scheduler).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn store_refusal_midstream_flush_ends_worker_without_waiting_for_another_event() {
  let project = project(&Setup::new("Keep working."));
  let root = project.repo.path();
  let raw = rusqlite::Connection::open(root.join(".orchester/orchester.db")).expect("fixture db");
  raw
    .execute_batch(&REFUSE_END_ONCE.replace("NEW.ended_at IS NOT NULL", "NEW.session_id IS NOT NULL"))
    .expect("refuse session flush once");
  let (scheduler, mut events) = scheduler(root);
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("start");
  let ended = wait_for(root, &mut events, id, |run| run.status() != &RunStatus::Running).await;
  wait_for_idle(&scheduler).await;
  assert_eq!(ended.status(), &RunStatus::Interrupted(InterruptCause::RunnerFailed));
  assert_failure_recorded(root, id);
  let calls = ProjectDb::open(root).expect("db").invocations(id).expect("invocations");
  assert_eq!(calls.len(), 1);
  assert!(calls[0].ended_at.is_some());
  assert_eq!(calls[0].outcome, Some(InvocationOutcome::Unrecorded));
  assert!(
    calls[0].session.is_none(),
    "the failed session write was not fabricated"
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn store_refusal_persistent_end_failure_stops_peers_and_keeps_honest_interruption() {
  let project = project(&Setup {
    steps: vec![
      StepSpec::new("implement", "Implement", "Work slowly. Write own file.", &[]),
      StepSpec::new("peer", "Peer", "Keep working.", &[]),
    ],
    ..Setup::new("Complete independent steps.")
  });
  let root = project.repo.path();
  let raw = rusqlite::Connection::open(root.join(".orchester/orchester.db")).expect("fixture db");
  raw
    .execute_batch(
      "CREATE TRIGGER refuse_end BEFORE UPDATE ON invocations \
    WHEN NEW.ended_at IS NOT NULL BEGIN SELECT RAISE(FAIL, 'worker end refused by the test'); END;",
    )
    .expect("persistent refusal, including peer end and cleanup repair");
  let (scheduler, mut events) = scheduler(root);
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("start");
  let ended = wait_for(root, &mut events, id, |run| run.status() != &RunStatus::Running).await;
  wait_for_idle(&scheduler).await;
  assert_eq!(ended.status(), &RunStatus::Interrupted(InterruptCause::RunnerFailed));
  assert_failure_recorded(root, id);
  let db = ProjectDb::open(root).expect("db");
  let calls = db.invocations(id).expect("invocations");
  assert_eq!(calls.len(), 2, "both parallel workers started");
  assert!(
    calls.iter().all(|call| call.ended_at.is_none()),
    "refused end writes remain unknown"
  );
  assert_stopped_peers(&db, id);
  let (continued, refused) = scheduler.continue_runs(&[id]).await;
  assert!(continued.is_empty(), "no launch while repair is refused");
  assert_eq!(refused.len(), 1);
  assert_eq!(db.invocations(id).expect("no new invocation"), calls);
  let interrupted_at = db.events(id).expect("history").last().expect("interruption").at();
  raw
    .execute("DROP TRIGGER refuse_end", [])
    .expect("restore writable fixture");
  let (continued, refused) = scheduler.continue_runs(&[id]).await;
  assert_eq!(continued, [id]);
  assert!(refused.is_empty(), "continuation: {refused:?}");
  let recovered = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  wait_for_idle(&scheduler).await;
  assert_eq!(recovered.status(), &RunStatus::Succeeded);
  assert_eq!(recovered.branch(), ended.branch());
  let repaired = db.invocations(id).expect("repaired and resumed");
  assert_eq!(repaired.len(), 4);
  for (old, closed) in calls.iter().zip(&repaired) {
    assert_eq!(closed.id, old.id);
    assert_eq!(closed.session, old.session);
    assert_eq!(closed.usage, old.usage);
    assert_eq!(closed.ended_at, Some(interrupted_at));
    assert_eq!(closed.outcome, Some(InvocationOutcome::Unrecorded));
  }
  assert_eq!(
    active_time(&repaired[..2], Timestamp::now()),
    active_time(&repaired[..2], interrupted_at)
  );
  tokio::time::timeout(support::PATIENCE, scheduler.stop_all(InterruptCause::QuitNow))
    .await
    .expect("immediate shutdown stays bounded");
}

fn assert_stopped_peers(db: &ProjectDb, id: orchester_core::id::RunId) {
  for pid in support::reported(db, id, "pid: ")
    .into_iter()
    .chain(support::reported(db, id, "helper pid: "))
  {
    assert!(!support::is_running(&pid), "owned peer process {pid} stopped");
  }
}

fn assert_failure_recorded(root: &std::path::Path, id: orchester_core::id::RunId) {
  let activity = ProjectDb::open(root).expect("db").activity(id, 0).expect("activity");
  assert!(
    activity
      .iter()
      .any(|entry| matches!(&entry.event, AgentEvent::RunnerProblem {
    problem: RunnerProblem::RunnerFailed { detail },
  } if detail.contains("worker end refused by the test"))),
    "original failure retained"
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn store_refusal_interrupts_the_run_and_reports_why() {
  let project = project(&Setup::new("Append one line."));
  let root = project.repo.path();
  let raw = rusqlite::Connection::open(root.join(".orchester").join("orchester.db")).expect("open");
  raw.execute(REFUSE_OUTPUTS, []).expect("the trigger");
  let (scheduler, mut events) = scheduler(root);
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");
  let run = wait_for(root, &mut events, id, |run| run.status() != &RunStatus::Running).await;
  assert_eq!(run.status(), &RunStatus::Interrupted(InterruptCause::RunnerFailed));
  let reported = ProjectDb::open(root)
    .expect("open")
    .activity(id, 0)
    .expect("activity")
    .into_iter()
    .any(|entry| {
      matches!(entry.event, AgentEvent::RunnerProblem {
        problem: RunnerProblem::RunnerFailed { detail },
      } if detail.contains("refused by the test"))
    });
  assert!(reported, "the run's activity says why it stopped");
}
