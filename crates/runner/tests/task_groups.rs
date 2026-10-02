#[path = "task_groups/blocked.rs"]
mod blocked;
#[path = "task_groups/reopen.rs"]
mod reopen;
mod support;
use std::num::NonZeroU32;

use orchester_core::{
  connection::AgentProvider,
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
  db.queue_task_group(&group, None, &reviewed(&db, &group))
    .expect("queue");
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
  assert!(
    db.unfinished_task_groups().expect("groups").is_empty(),
    "a group whose runs all ended is finished"
  );
}

/// Waits until the scheduler has nothing to do, as `StopIfIdle` asks it.
async fn becomes_idle(scheduler: &orchester_runner::execution::Scheduler) {
  let start = std::time::Instant::now();
  while !scheduler.is_idle() {
    assert!(
      start.elapsed() < std::time::Duration::from_secs(20),
      "the runner stays busy"
    );
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
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
  db.queue_task_group(&group, None, &reviewed(db, &group)).expect("queue");
  group
}

#[tokio::test(flavor = "multi_thread")]
async fn task_group_shared_cap_stops_the_run_and_holds_unstarted_tasks() {
  let fixture = project(&Setup {
    provider: AgentProvider::ClaudeCodeCli,
    ..Setup::new("Work in turns.")
  });
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
  // The spent cap holds the second task until a person acts, so the runner may stop.
  becomes_idle(&scheduler).await;
  // Reopening the project keeps that hold: the second task can still go back to the drafts.
  drop(scheduler);
  let (fresh, _events) = support::scheduler(root);
  fresh
    .attach(
      AttachSession::Fresh {
        previous_end: orchester_ipc::SessionEnd::Clean,
      },
      NonZeroU32::new(2).expect("slots"),
    )
    .await
    .expect("attach");
  assert_eq!(
    db.task_group_hold(group.key()).expect("hold"),
    Some(GroupHold::GroupCap)
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn task_group_paused_member_leaves_the_runner_idle() {
  // Two steps, so the pause settles between them.
  let fixture = project(&Setup {
    review: true,
    ..Setup::new("Work slowly.")
  });
  let root = fixture.repo.path();
  let mut db = ProjectDb::open(root).expect("db");
  let group = reviewed_group(&mut db, fixture.task, Scheduling::Sequential, 3_000_000);
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
  assert!(!scheduler.is_idle(), "a running member keeps the runner busy");
  wait_for_turns(root, &mut events, first.id(), 1).await;
  scheduler.pause_run(first.id()).await.expect("pause");
  assert!(!scheduler.is_idle(), "a member still pausing keeps the runner busy");
  wait_for(root, &mut events, first.id(), |run| {
    run.status() == &RunStatus::Paused(PauseReason::Requested)
  })
  .await;
  scheduler.advance_task_groups().await.expect("advance");
  // The second task waits for the paused one, which only a person continues.
  becomes_idle(&scheduler).await;
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
  let fixture = project(&Setup {
    provider: AgentProvider::ClaudeCodeCli,
    ..Setup::new("Work in turns.")
  });
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
  // The cancelled task holds the second one for good: the runner is idle, and the person can
  // take it back to the drafts, after which the group is finished.
  becomes_idle(&scheduler).await;
  db.release_held_tasks(&[group.tasks()[1]]).expect("release");
  scheduler.advance_task_groups().await.expect("advance released group");
  assert!(db.unfinished_task_groups().expect("groups").is_empty());
}

fn reviewed(db: &ProjectDb, group: &TaskGroup) -> Vec<orchester_core::task_group::ReviewedPipeline> {
  use orchester_core::{resource::Pipeline, store::ResourceStore, task_group::ReviewedPipeline};
  group
    .tasks()
    .iter()
    .map(|id| {
      let task = db.task(*id).expect("task").expect("exists");
      let pipeline = task.details().pipeline.expect("pipeline");
      let version = db
        .latest_version::<Pipeline>(pipeline)
        .expect("version")
        .expect("published")
        .number();
      ReviewedPipeline {
        task: *id,
        pipeline,
        version,
      }
    })
    .collect()
}
