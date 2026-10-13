//! Requests already queued while failure cleanup runs must receive its explicit failure;
//! they must not be silently retried against the subsequently interrupted run.
#[path = "support/failure_hold.rs"]
mod failure_hold;
mod support;

use std::{
  future::{Future, poll_fn},
  pin::pin,
  task::Poll,
};

use orchester_core::{
  id::IdempotencyKey,
  run::{InterruptCause, RunStatus},
};
use orchester_ipc::Rejection;
use support::{PATIENCE, Setup, StepSpec, project, scheduler, wait_for, wait_for_idle};

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn worker_failure_controls_receive_original_error_and_shutdown_finishes() {
  let hold = failure_hold::hold_cleanup();
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
      "CREATE TRIGGER refuse_end BEFORE UPDATE ON invocations WHEN NEW.ended_at IS NOT NULL \
    BEGIN SELECT RAISE(FAIL, 'worker end refused by the test'); END;",
    )
    .expect("inject failure");
  let (scheduler, mut events) = scheduler(root);
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("start");
  tokio::time::timeout(PATIENCE, hold.held)
    .await
    .expect("cleanup begins")
    .expect("held");
  let mut pause = pin!(scheduler.pause_run(id));
  let mut cancel = pin!(scheduler.cancel_run(id));
  let mut messages = pin!(scheduler.deliver_messages(id));
  // First polling each public request enqueues its control and waits for its acknowledgment.
  poll_fn(|cx| {
    assert!(pause.as_mut().poll(cx).is_pending());
    assert!(cancel.as_mut().poll(cx).is_pending());
    assert!(messages.as_mut().poll(cx).is_pending());
    Poll::Ready(())
  })
  .await;
  hold.release.send(()).expect("release cleanup");
  let replies = tokio::time::timeout(PATIENCE, async { tokio::join!(pause, cancel, messages) })
    .await
    .expect("all controls answered during cleanup");
  for result in [replies.0, replies.1, replies.2] {
    assert!(matches!(result, Err(Rejection::Failed { detail }) if detail.contains("worker end refused by the test")));
  }
  let ended = wait_for(root, &mut events, id, |run| run.status() != &RunStatus::Running).await;
  wait_for_idle(&scheduler).await;
  assert_eq!(ended.status(), &RunStatus::Interrupted(InterruptCause::RunnerFailed));
  tokio::time::timeout(PATIENCE, scheduler.stop_all(InterruptCause::QuitNow))
    .await
    .expect("bounded immediate shutdown");
}
