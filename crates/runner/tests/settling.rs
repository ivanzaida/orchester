//! A request to continue a run that arrives once the run is recorded as stopped, but before the
//! task that stopped it has ended: the desktop reads the stop as soon as it is announced, while
//! the task still finishes, which on a busy machine can take a while. The run no longer counts as
//! executing, and the request waits for the task instead of being refused as if the run were
//! still executing. The task is held at its last step through the runner's log, so the moment
//! does not depend on timing. Needs `git`. Run with
//! `cargo test -p orchester-runner --features test-agent --test settling`.

mod support;

use std::{future::Future, task::Poll, time::Duration};

use orchester_core::{
  id::IdempotencyKey,
  run::{PauseReason, RaisedLimits, RunLimit, RunStatus},
  store::RunStore,
  tokens::TokenBudget,
};
use orchester_store::ProjectDb;

use self::support::{PATIENCE, Setup, hold::hold_first_settled, project, scheduler, wait_for};

#[tokio::test(flavor = "multi_thread")]
async fn settling_a_raise_waits_for_the_task_that_stopped_the_run() {
  let hold = hold_first_settled();
  let project = project(&Setup {
    budget: 5_000,
    ..Setup::new("Work in turns.")
  });
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .expect("the run starts");
  tokio::time::timeout(PATIENCE, hold.held)
    .await
    .expect("the run stops at its token limit in time")
    .expect("the task is held");
  let stopped = ProjectDb::open(root)
    .expect("open")
    .run(id)
    .expect("read")
    .expect("run");
  assert_eq!(
    stopped.status(),
    &RunStatus::Paused(PauseReason::LimitReached(RunLimit::Tokens))
  );
  // The task only ends now: the stopped run no longer counts as executing.
  assert!(scheduler.active_runs().is_empty());

  let limits = RaisedLimits {
    budget: TokenBudget::new(2_000_000).expect("above zero"),
    max_duration: Duration::from_secs(30 * 60),
  };
  let raise = scheduler.raise_limit(id, limits);
  tokio::pin!(raise);
  let first = std::future::poll_fn(|cx| Poll::Ready(raise.as_mut().poll(cx))).await;
  assert!(first.is_pending(), "the raise waits for the task: {first:?}");
  hold.release.send(()).expect("the task is held");
  assert_eq!(raise.await, Ok(()));
  let done = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(done.status(), &RunStatus::Succeeded);
}
