//! Requests for one run share the scheduler lock while a continuation records its new task.
//! An SQLite writer holds the continuation inside that lock, making the overlap observable.

mod support;

use std::{future::Future, num::NonZeroU32, task::Poll, time::Duration};

use orchester_core::{
  id::{IdempotencyKey, ResourceId, RunId, StepExecutionId, TaskId},
  permission::{AgentPermissions, CommandPermission, NetworkAccess, RepositoryAccess},
  run::{PauseReason, PermissionsApply, PlanDecision, PlanVersion, RaisedLimits, RunLimit, RunStatus},
  store::RunStore,
  tokens::TokenBudget,
};
use orchester_ipc::{AttachSession, Event, SessionEnd};
use orchester_runner::execution::Scheduler;
use orchester_store::ProjectDb;
use rusqlite::Connection;
use tokio::sync::mpsc;

use self::support::{PATIENCE, Setup, project, scheduler, wait_for, wait_for_idle};

async fn assert_pending_once<F: Future>(future: F, request: &str) {
  tokio::pin!(future);
  let first = std::future::poll_fn(|cx| Poll::Ready(future.as_mut().poll(cx))).await;
  assert!(first.is_pending(), "{request} waits for the same scheduler lock");
}

async fn stop_at_limit(
  project: &support::TestProject,
) -> (Scheduler, mpsc::UnboundedReceiver<Event>, RunId, ResourceId) {
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let run = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");
  let stopped = wait_for(root, &mut events, run, |run| {
    run.status() == &RunStatus::Paused(PauseReason::LimitReached(RunLimit::Tokens))
  })
  .await;
  assert_eq!(
    stopped.status(),
    &RunStatus::Paused(PauseReason::LimitReached(RunLimit::Tokens))
  );
  let step = stopped.snapshot().steps()[0].key().clone();
  let builder = stopped.profile_of(&step).expect("the run has an agent profile");
  wait_for_idle(&scheduler).await;
  (scheduler, events, run, builder)
}

async fn assert_other_requests_wait(scheduler: &Scheduler, run: RunId, builder: ResourceId) {
  assert_pending_once(scheduler.continue_runs(&[run]), "continue_runs").await;
  assert_pending_once(
    scheduler.start_run(IdempotencyKey::generate(), TaskId::generate()),
    "start_run",
  )
  .await;
  assert_pending_once(scheduler.pause_run(run), "pause_run").await;
  assert_pending_once(scheduler.deliver_messages(run), "deliver_messages").await;
  assert_pending_once(
    scheduler.decide_plan(
      run,
      StepExecutionId::generate(),
      PlanVersion::FIRST,
      PlanDecision::Approved,
    ),
    "decide_plan",
  )
  .await;
  let read_only = AgentPermissions::new(
    RepositoryAccess::ReadOnly,
    CommandPermission::AnyInWorktree,
    NetworkAccess::Off,
  );
  assert_pending_once(
    scheduler.change_permissions(run, builder, read_only, PermissionsApply::NextInvocation),
    "change_permissions",
  )
  .await;
  assert_pending_once(
    scheduler.keep_permissions_for_next_invocation(run, builder),
    "keep_permissions_for_next_invocation",
  )
  .await;
  assert_pending_once(
    scheduler.attach(
      AttachSession::Fresh {
        previous_end: SessionEnd::Clean,
      },
      NonZeroU32::MIN,
    ),
    "attach",
  )
  .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn requests_wait_while_a_limit_raise_starts_the_next_task() {
  let project = project(&Setup {
    budget: 5_000,
    ..Setup::new("Work in turns.")
  });
  let root = project.repo.path();
  let (scheduler, mut events, run, builder) = stop_at_limit(&project).await;

  std::fs::write(root.join(".orchester").join("fake-hold"), "hold resumed agent").expect("hold the next invocation");
  let database = root.join(".orchester").join("orchester.db");
  let writer = Connection::open(database).expect("open the project database");
  writer.execute_batch("BEGIN IMMEDIATE").expect("hold the write lock");

  let limits = RaisedLimits {
    budget: TokenBudget::new(2_000_000).expect("above zero"),
    max_duration: Duration::from_secs(30 * 60),
  };
  let continuing = scheduler.clone();
  let raise = tokio::spawn(async move { continuing.raise_limit(run, limits).await });
  tokio::time::timeout(PATIENCE, async {
    while scheduler.is_idle() {
      tokio::task::yield_now().await;
    }
  })
  .await
  .expect("the raise acquires the scheduler lock");
  assert!(
    scheduler.active_runs().is_empty(),
    "the continuation has not started a task yet"
  );

  assert_other_requests_wait(&scheduler, run, builder).await;

  let cancel = scheduler.cancel_run(run);
  tokio::pin!(cancel);
  assert_pending_once(cancel.as_mut(), "cancel_run").await;

  writer.execute_batch("ROLLBACK").expect("release the write lock");
  tokio::time::timeout(PATIENCE, raise)
    .await
    .expect("the raise answers in time")
    .expect("the request task completes")
    .expect("the limit is raised");
  let resumed = ProjectDb::open(root)
    .expect("open")
    .run(run)
    .expect("read")
    .expect("run");
  assert_eq!(resumed.status(), &RunStatus::Running);
  tokio::time::timeout(PATIENCE, cancel)
    .await
    .expect("the cancel answers in time")
    .expect("cancel the resumed run");
  let cancelled = wait_for(root, &mut events, run, |run| run.status() == &RunStatus::Cancelled).await;
  assert_eq!(cancelled.status(), &RunStatus::Cancelled);
  std::fs::remove_file(root.join(".orchester").join("fake-hold")).expect("remove the fake hold");
}
