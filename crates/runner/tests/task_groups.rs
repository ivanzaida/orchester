mod support;
use std::num::NonZeroU32;

use orchester_core::{
  id::IdempotencyKey,
  run::{InterruptCause, PauseReason, RunLimit, RunStatus},
  store::{RunStore, TaskStore},
  task_group::{GroupHold, Scheduling, TaskGroup},
  time::Timestamp,
  tokens::TokenBudget,
};
use orchester_ipc::{AttachSession, Rejection};
use orchester_store::ProjectDb;
use support::{Setup, project, scheduler, wait_for, wait_for_turns};

async fn run_group(mode: Scheduling) {
  let fixture = project(&Setup::new("Append one line."));
  let root = fixture.repo.path();
  let mut db = ProjectDb::open(root).expect("open");
  let details = db.task(fixture.task).expect("task").expect("exists").details().clone();
  let drafts = db
    .create_tasks(
      IdempotencyKey::generate(),
      vec![details.clone(), details],
      Timestamp::now(),
    )
    .expect("drafts");
  let group = TaskGroup::new(
    IdempotencyKey::generate(),
    drafts.iter().map(|t| t.id()).collect(),
    mode,
    TokenBudget::new(3_000_000).expect("cap"),
  )
  .expect("group");
  db.queue_task_group(&group, None).expect("queue");
  let (scheduler, mut events) = scheduler(root);
  scheduler
    .attach(AttachSession::Reconnect, NonZeroU32::new(4).expect("slots"))
    .await
    .expect("attach");
  scheduler.advance_task_groups().await.expect("advance");
  let first = db
    .run_with_key(drafts[0].id().first_run_key())
    .expect("first")
    .expect("started");
  if mode == Scheduling::Sequential {
    assert!(
      db.run_with_key(drafts[1].id().first_run_key())
        .expect("second")
        .is_none()
    );
  }
  wait_for(root, &mut events, first.id(), |run| {
    run.status() == &RunStatus::Succeeded
  })
  .await;
  scheduler.advance_task_groups().await.expect("advance");
  let second = db
    .run_with_key(drafts[1].id().first_run_key())
    .expect("second")
    .expect("started");
  wait_for(root, &mut events, second.id(), |run| {
    run.status() == &RunStatus::Succeeded
  })
  .await;
  scheduler.advance_task_groups().await.expect("retry advance");
  for task in drafts {
    assert_eq!(db.runs_of_task(task.id()).expect("runs").len(), 1);
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn task_group_sequential_start_and_retry_are_idempotent() {
  run_group(Scheduling::Sequential).await;
}
#[tokio::test(flavor = "multi_thread")]
async fn task_group_parallel_start_and_retry_are_idempotent() {
  run_group(Scheduling::Parallel(NonZeroU32::new(2).expect("positive"))).await;
}

fn reviewed_group(db: &mut ProjectDb, original: orchester_core::id::TaskId, mode: Scheduling, cap: u64) -> TaskGroup {
  let details = db.task(original).expect("task").expect("exists").details().clone();
  let drafts = db
    .create_tasks(
      IdempotencyKey::generate(),
      vec![details.clone(), details],
      Timestamp::now(),
    )
    .expect("drafts");
  let group = TaskGroup::new(
    IdempotencyKey::generate(),
    drafts.iter().map(|t| t.id()).collect(),
    mode,
    TokenBudget::new(cap).expect("cap"),
  )
  .expect("group");
  db.queue_task_group(&group, None).expect("queue");
  group
}

#[tokio::test(flavor = "multi_thread")]
async fn task_group_shared_cap_stops_the_run_and_holds_unstarted_tasks() {
  let fixture = project(&Setup::new("Work in turns."));
  let root = fixture.repo.path();
  let mut db = ProjectDb::open(root).expect("db");
  let group = reviewed_group(&mut db, fixture.task, Scheduling::Sequential, 5_000);
  let (scheduler, mut events) = scheduler(root);
  scheduler
    .attach(AttachSession::Reconnect, NonZeroU32::new(2).expect("slots"))
    .await
    .expect("attach");
  assert_eq!(
    scheduler
      .start_run(group.tasks()[1].first_run_key(), group.tasks()[1])
      .await,
    Err(Rejection::TaskGroupWaiting {
      hold: GroupHold::PreviousTask
    })
  );
  scheduler.advance_task_groups().await.expect("advance");
  let first = db
    .run_with_key(group.tasks()[0].first_run_key())
    .expect("read")
    .expect("started");
  let stopped = wait_for(root, &mut events, first.id(), |run| {
    matches!(run.status(), RunStatus::Paused(_))
  })
  .await;
  assert_eq!(
    stopped.status(),
    &RunStatus::Paused(PauseReason::LimitReached(RunLimit::GroupBudget))
  );
  scheduler.advance_task_groups().await.expect("advance after cap");
  assert!(
    db.run_with_key(group.tasks()[1].first_run_key())
      .expect("read")
      .is_none()
  );
  assert_eq!(
    db.task_group_hold(group.key()).expect("hold"),
    Some(GroupHold::GroupCap)
  );
  assert_eq!(
    scheduler
      .start_run(group.tasks()[1].first_run_key(), group.tasks()[1])
      .await,
    Err(Rejection::TaskGroupWaiting {
      hold: GroupHold::GroupCap
    })
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn task_group_parallel_still_obeys_runner_agent_slots() {
  let fixture = project(&Setup::new("Keep working."));
  let root = fixture.repo.path();
  let mut db = ProjectDb::open(root).expect("db");
  let group = reviewed_group(
    &mut db,
    fixture.task,
    Scheduling::Parallel(NonZeroU32::new(2).expect("concurrency")),
    3_000_000,
  );
  let (scheduler, _events) = scheduler(root);
  scheduler
    .attach(AttachSession::Reconnect, NonZeroU32::new(1).expect("slots"))
    .await
    .expect("attach");
  scheduler.advance_task_groups().await.expect("advance");
  assert!(
    db.run_with_key(group.tasks()[0].first_run_key())
      .expect("read")
      .is_some()
  );
  assert!(
    db.run_with_key(group.tasks()[1].first_run_key())
      .expect("read")
      .is_none()
  );
  assert_eq!(
    db.task_group_hold(group.key()).expect("hold"),
    Some(GroupHold::AgentCapacity)
  );
  scheduler.stop_all(InterruptCause::QuitNow).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn task_group_cancel_then_run_again_does_not_inherit_exhausted_shared_cap() {
  let fixture = project(&Setup::new("Work in turns."));
  let root = fixture.repo.path();
  let mut db = ProjectDb::open(root).expect("db");
  let group = reviewed_group(&mut db, fixture.task, Scheduling::Sequential, 5_000);
  let (scheduler, mut events) = scheduler(root);
  scheduler
    .attach(AttachSession::Reconnect, NonZeroU32::new(2).expect("slots"))
    .await
    .expect("attach");
  scheduler.advance_task_groups().await.expect("advance");
  let first = db
    .run_with_key(group.tasks()[0].first_run_key())
    .expect("first")
    .expect("started");
  let stopped = wait_for(root, &mut events, first.id(), |run| {
    matches!(run.status(), RunStatus::Paused(_))
  })
  .await;
  assert_eq!(
    stopped.status(),
    &RunStatus::Paused(PauseReason::LimitReached(RunLimit::GroupBudget))
  );
  let group_usage = db.task_group_usage(&group).expect("usage");
  scheduler.cancel_run(first.id()).await.expect("cancel");
  let key = IdempotencyKey::generate();
  let next = scheduler
    .run_again(key, first.task(), first.id(), None)
    .await
    .expect("run again");
  let advanced = wait_for(root, &mut events, next, |run| {
    run.used().get() >= 5_000 || matches!(run.status(), RunStatus::Paused(_))
  })
  .await;
  assert_eq!(
    advanced.status(),
    &RunStatus::Running,
    "later run starts and works beyond the old shared cap"
  );
  wait_for_turns(root, &mut events, next, 2).await;
  assert!(
    !db
      .task_group_limit_reached(&advanced)
      .expect("new run is outside group")
  );
  assert_eq!(db.task_group_usage(&group).expect("original group usage"), group_usage);
  assert_eq!(
    scheduler
      .run_again(key, first.task(), first.id(), None)
      .await
      .expect("retry"),
    next
  );
  scheduler.cancel_run(next).await.expect("cancel later run");
  scheduler.advance_task_groups().await.expect("advance held group");
  assert!(
    db.run_with_key(group.tasks()[1].first_run_key())
      .expect("second")
      .is_none()
  );
  assert_eq!(db.runs_of_task(first.task()).expect("runs").len(), 2);
}
