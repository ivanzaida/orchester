//! Groups with a task that became blocked after it was queued: the group passes over it, starts
//! the tasks after it, waits for the blocking tasks without keeping the runner busy, and starts
//! the blocked task once they finish, even when one of them is a later task of the same group.
use std::num::NonZeroU32;

use orchester_core::{
  id::TaskId,
  run::RunStatus,
  store::RunStore,
  task::organization::{BlockingTask, ChangeActor, DependencyGate, Priority, TaskOrganization, WorkflowStatus},
  task_group::{GroupHold, Scheduling},
  time::Timestamp,
};
use orchester_ipc::AttachSession;
use orchester_store::ProjectDb;

use super::{
  becomes_idle, reviewed_group,
  support::{Setup, project, scheduler, wait_for},
};

/// Links `task` to `blocker`, which it waits for until `blocker`'s run is ready for review.
fn block(db: &mut ProjectDb, task: TaskId, blocker: TaskId) {
  let value = TaskOrganization::new(
    WorkflowStatus::Todo,
    Priority::None,
    Some("Waits for the other task.".into()),
    vec![BlockingTask {
      task: blocker,
      gate: DependencyGate::Review,
    }],
    vec![],
  )
  .expect("organization");
  db.set_task_organization(task, &value, ChangeActor::Person, Timestamp::now())
    .expect("link");
}

/// The group's first task waits for a draft outside the group; the second runs to the end.
async fn blocked_by_a_task_outside(mode: Scheduling) {
  let fixture = project(&Setup::new("Append one line."));
  let root = fixture.repo.path();
  let mut db = ProjectDb::open(root).expect("db");
  let group = reviewed_group(&mut db, fixture.task, mode, 3_000_000);
  let (blocked, free) = (group.tasks()[0], group.tasks()[1]);
  block(&mut db, blocked, fixture.task);
  let (scheduler, mut events) = scheduler(root);
  scheduler
    .attach(AttachSession::Reconnect, NonZeroU32::new(4).expect("slots"))
    .await
    .expect("attach");
  scheduler.advance_task_groups().await.expect("advance");
  assert!(db.run_with_key(blocked.first_run_key()).expect("read").is_none());
  let run = db
    .run_with_key(free.first_run_key())
    .expect("read")
    .expect("the task after the blocked one starts");
  wait_for(root, &mut events, run.id(), |run| run.status() == &RunStatus::Succeeded).await;
  scheduler.advance_task_groups().await.expect("advance");
  assert!(db.run_with_key(blocked.first_run_key()).expect("read").is_none());
  assert_eq!(
    db.task_group_hold(group.key()).expect("hold"),
    Some(GroupHold::Blocked { by: vec![fixture.task] })
  );
  // The blocking draft has no run that would finish it: nothing keeps the runner busy.
  becomes_idle(&scheduler).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn task_group_parallel_passes_over_a_blocked_task_and_waits_idle() {
  blocked_by_a_task_outside(Scheduling::Parallel(NonZeroU32::new(2).expect("two"))).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn task_group_sequential_passes_over_a_blocked_task_and_waits_idle() {
  blocked_by_a_task_outside(Scheduling::Sequential).await;
}

/// The first task waits for the second task of the same sequential group: the second runs first,
/// then the first, and the group finishes.
#[tokio::test(flavor = "multi_thread")]
async fn task_group_blocker_inside_the_group_runs_first_without_deadlock() {
  let fixture = project(&Setup::new("Append one line."));
  let root = fixture.repo.path();
  let mut db = ProjectDb::open(root).expect("db");
  let group = reviewed_group(&mut db, fixture.task, Scheduling::Sequential, 3_000_000);
  let (first, second) = (group.tasks()[0], group.tasks()[1]);
  block(&mut db, first, second);
  let (scheduler, mut events) = scheduler(root);
  scheduler
    .attach(AttachSession::Reconnect, NonZeroU32::new(4).expect("slots"))
    .await
    .expect("attach");
  scheduler.advance_task_groups().await.expect("advance");
  assert!(db.run_with_key(first.first_run_key()).expect("read").is_none());
  let blocker = db
    .run_with_key(second.first_run_key())
    .expect("read")
    .expect("the blocking task starts first");
  wait_for(root, &mut events, blocker.id(), |run| {
    run.status() == &RunStatus::Succeeded
  })
  .await;
  scheduler.advance_task_groups().await.expect("advance");
  let unblocked = db
    .run_with_key(first.first_run_key())
    .expect("read")
    .expect("the blocked task starts once its blocking task is ready for review");
  wait_for(root, &mut events, unblocked.id(), |run| {
    run.status() == &RunStatus::Succeeded
  })
  .await;
  scheduler.advance_task_groups().await.expect("advance");
  assert!(db.unfinished_task_groups().expect("groups").is_empty());
}
