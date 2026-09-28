//! A run whose record cannot be read back (edited or damaged outside Orchester): attaching and
//! recovering carry on with the other runs and leave it as it is, and every request for it is
//! rejected as unreadable. Against `orchester-fake-codex`; needs `git`. Run with
//! `cargo test -p orchester-runner --features test-agent --test unreadable`.

mod support;

use std::{num::NonZeroU32, path::Path};

use orchester_core::{
  id::RunId,
  run::{InterruptCause, RunStatus},
  store::RunStore,
  time::Timestamp,
};
use orchester_ipc::{AttachSession, Rejection};
use orchester_runner::execution::Scheduler;
use orchester_store::ProjectDb;
use rusqlite::Connection;

use self::support::{PATIENCE, Setup, killed_mid_run, project, queue_another_task, scheduler, wait_for};

fn slots(count: u32) -> NonZeroU32 {
  NonZeroU32::new(count).expect("above zero")
}

fn raw(root: &Path) -> Connection {
  Connection::open(root.join(".orchester/orchester.db")).expect("open raw database")
}

/// Sets `assignments` on the row of `run`, as an edit or damage outside Orchester would.
fn tamper(root: &Path, run: RunId, assignments: &str) {
  raw(root)
    .execute(
      &format!("UPDATE runs SET {assignments} WHERE id = ?1"),
      [run.to_string()],
    )
    .expect("tamper");
}

/// Everything stored about `run`: its row and how many events, attempts and executions it has.
fn stored(root: &Path, run: RunId) -> (String, String, i64, i64, i64) {
  raw(root)
    .query_row(
      "SELECT status, snapshot, last_sequence, \
       (SELECT count(*) FROM run_events WHERE run_id = runs.id), \
       (SELECT count(*) FROM attempts WHERE run_id = runs.id) \
       FROM runs WHERE id = ?1",
      [run.to_string()],
      |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
    )
    .expect("stored run")
}

/// Every run request for `run` is rejected because it cannot be read.
async fn assert_requests_rejected(scheduler: &Scheduler, run: RunId) {
  let unreadable = |result: Result<(), Rejection>| matches!(result, Err(Rejection::RunUnreadable { .. }));
  let answered = async {
    assert!(unreadable(scheduler.pause_run(run).await), "pause");
    assert!(unreadable(scheduler.resume_run(run).await), "resume");
    assert!(unreadable(scheduler.cancel_run(run).await), "cancel");
  };
  tokio::time::timeout(PATIENCE, answered)
    .await
    .expect("answered in time");
  let (continued, refused) = scheduler.continue_runs(&[run]).await;
  assert!(continued.is_empty());
  assert!(matches!(refused[0].rejection, Rejection::RunUnreadable { .. }));
}

#[tokio::test(flavor = "multi_thread")]
async fn unreadable_running_run_is_left_as_it_is_when_the_runner_attaches() {
  let project = project(&Setup::new("Keep working."));
  let root = project.repo.path();
  let other = queue_another_task(root);
  let good = killed_mid_run(root, project.task).await;
  let bad = killed_mid_run(root, other).await;
  tamper(root, bad, "snapshot = 'not json'");
  let before = stored(root, bad);

  let (scheduler, mut events) = scheduler(root);
  let attached = scheduler
    .attach(AttachSession::Reconnect, slots(2))
    .await
    .expect("one unreadable run does not fail the attach");
  assert_eq!(attached.interrupted, vec![good]);
  assert_eq!(attached.continued, vec![good]);
  wait_for(root, &mut events, good, |run| run.status() == &RunStatus::Succeeded).await;

  assert_requests_rejected(&scheduler, bad).await;
  assert_eq!(stored(root, bad), before, "the unreadable run is never written over");
}

#[tokio::test(flavor = "multi_thread")]
async fn unreadable_interrupted_run_is_not_continued_after_a_reconnect() {
  let project = project(&Setup::new("Keep working."));
  let root = project.repo.path();
  let other = queue_another_task(root);
  let runs = [
    killed_mid_run(root, project.task).await,
    killed_mid_run(root, other).await,
  ];
  // Both were interrupted by the loss of a runner, which continues them on a reconnect.
  let mut db = ProjectDb::open(root).expect("open");
  for id in runs {
    let mut run = db.run(id).expect("read").expect("run");
    let events = run
      .interrupt(InterruptCause::RunnerLost, Timestamp::now())
      .expect("interrupt");
    db.append_events(&run, &events).expect("append");
  }
  let [good, bad] = runs;
  tamper(root, bad, "status_reason = 'not a cause'");
  let before = stored(root, bad);

  let (scheduler, mut events) = scheduler(root);
  let attached = scheduler
    .attach(AttachSession::Reconnect, slots(2))
    .await
    .expect("one unreadable run does not fail the recovery");
  assert!(attached.interrupted.is_empty());
  assert_eq!(attached.continued, vec![good]);
  wait_for(root, &mut events, good, |run| run.status() == &RunStatus::Succeeded).await;

  assert_requests_rejected(&scheduler, bad).await;
  assert_eq!(stored(root, bad), before, "the unreadable run is never written over");
}
