//! Groups across a session end: the reopened project holds unstarted tasks until a person starts
//! or continues the group's first run, which keeps its identity.
use std::num::NonZeroU32;

use orchester_core::{
  connection::AgentProvider,
  run::{InterruptCause, RunStatus},
  store::{RunStore, TaskStore},
  task_group::{GroupHold, Scheduling, TaskGroup},
  time::Timestamp,
};
use orchester_ipc::{AttachSession, Rejection};
use orchester_store::ProjectDb;

use super::{
  reviewed_group,
  support::{Setup, project, scheduler, wait_for, wait_for_turns},
};

#[tokio::test(flavor = "multi_thread")]
async fn task_group_fresh_reopen_holds_second_task_until_explicit_start() {
  let fixture = project(&Setup::new("Append one line."));
  let root = fixture.repo.path();
  let mut db = ProjectDb::open(root).expect("db");
  let group = reviewed_group(&mut db, fixture.task, Scheduling::Sequential, 3_000_000);
  let slots = NonZeroU32::new(2).expect("slots");
  let (original, mut events) = scheduler(root);
  original.attach(AttachSession::Reconnect, slots).await.expect("attach");
  original.advance_task_groups().await.expect("advance");
  let first = db
    .run_with_key(group.tasks()[0].first_run_key())
    .expect("run")
    .expect("started");
  let succeeded = wait_for(root, &mut events, first.id(), |run| {
    run.status() == &RunStatus::Succeeded
  })
  .await;
  original.stop_all(InterruptCause::QuitNow).await;
  drop(original);
  let second_task = group.tasks()[1];
  let (fresh, _events) = scheduler(root);
  fresh
    .attach(
      AttachSession::Fresh {
        previous_end: orchester_ipc::SessionEnd::QuitNow,
      },
      slots,
    )
    .await
    .expect("fresh");
  fresh.advance_task_groups().await.expect("held advance");
  assert!(db.run_with_key(second_task.first_run_key()).expect("second").is_none());
  assert_eq!(
    db.task_group_hold(group.key()).expect("hold"),
    Some(GroupHold::SessionEnded)
  );
  assert_eq!(
    fresh.start_run(second_task.first_run_key(), second_task).await,
    Err(Rejection::TaskGroupWaiting {
      hold: GroupHold::SessionEnded
    })
  );
  fresh.stop_all(InterruptCause::QuitNow).await;
  drop(fresh);
  let (reconnected, mut events) = scheduler(root);
  reconnected
    .attach(AttachSession::Reconnect, slots)
    .await
    .expect("reconnect");
  reconnected.advance_task_groups().await.expect("hold persists");
  assert!(db.run_with_key(second_task.first_run_key()).expect("second").is_none());
  let second = reconnected
    .start_requested_run(second_task.first_run_key(), second_task)
    .await
    .expect("explicit continuation");
  wait_for(root, &mut events, second, |run| run.status() == &RunStatus::Succeeded).await;
  assert_eq!(db.run(first.id()).expect("first").expect("exists"), succeeded);
  assert_eq!(db.task_group(group.key()).expect("group").expect("exists"), group);
  assert_eq!(
    reconnected
      .start_requested_run(second_task.first_run_key(), second_task)
      .await
      .expect("retry"),
    second
  );
  for task in group.tasks() {
    assert_eq!(db.runs_of_task(*task).expect("runs").len(), 1);
  }
}

/// A group queued after the runner started, before the desktop attached for a new session, is the
/// new session's: it is not held.
#[tokio::test(flavor = "multi_thread")]
async fn task_group_queued_while_the_runner_starts_is_not_held_by_a_fresh_attach() {
  let fixture = project(&Setup::new("Append one line."));
  let root = fixture.repo.path();
  let (fresh, mut events) = scheduler(root);
  let mut db = ProjectDb::open(root).expect("db");
  let group = reviewed_group(&mut db, fixture.task, Scheduling::Sequential, 3_000_000);
  fresh
    .attach(
      AttachSession::Fresh {
        previous_end: orchester_ipc::SessionEnd::QuitNow,
      },
      NonZeroU32::new(2).expect("slots"),
    )
    .await
    .expect("fresh");
  fresh.advance_task_groups().await.expect("advance");
  assert_ne!(
    db.task_group_hold(group.key()).expect("hold"),
    Some(GroupHold::SessionEnded)
  );
  let first = db
    .run_with_key(group.tasks()[0].first_run_key())
    .expect("run")
    .expect("started");
  wait_for(root, &mut events, first.id(), |run| {
    run.status() == &RunStatus::Succeeded
  })
  .await;
  fresh.stop_all(InterruptCause::QuitNow).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn task_group_fresh_reopen_explicit_continue_keeps_first_run_checkpoint() {
  let fixture = project(&Setup {
    provider: AgentProvider::ClaudeCodeCli,
    ..Setup::new("Work in turns.")
  });
  let root = fixture.repo.path();
  let mut db = ProjectDb::open(root).expect("db");
  let group = reviewed_group(&mut db, fixture.task, Scheduling::Sequential, 3_000_000);
  let slots = NonZeroU32::new(2).expect("slots");
  let (original, mut events) = scheduler(root);
  original.attach(AttachSession::Reconnect, slots).await.expect("attach");
  original.advance_task_groups().await.expect("advance");
  let first = db
    .run_with_key(group.tasks()[0].first_run_key())
    .expect("first")
    .expect("exists");
  wait_for_turns(root, &mut events, first.id(), 2).await;
  original.stop_all(InterruptCause::QuitNow).await;
  let interrupted = db.run(first.id()).expect("read").expect("exists");
  assert!(matches!(interrupted.status(), RunStatus::Interrupted(_)));
  drop(original);
  let (fresh, mut events) = scheduler(root);
  fresh
    .attach(
      AttachSession::Fresh {
        previous_end: orchester_ipc::SessionEnd::QuitNow,
      },
      slots,
    )
    .await
    .expect("fresh");
  fresh.advance_task_groups().await.expect("held");
  assert_eq!(db.run(first.id()).expect("run").expect("exists"), interrupted);
  assert_eq!(
    db.task_group_hold(group.key()).expect("hold"),
    Some(GroupHold::SessionEnded)
  );
  let (continued, refused) = fresh.continue_runs(&[first.id()]).await;
  assert_eq!(continued, vec![first.id()]);
  assert!(refused.is_empty());
  wait_for_turns(root, &mut events, first.id(), 3).await;
  assert_ne!(
    db.task_group_hold(group.key()).expect("hold"),
    Some(GroupHold::SessionEnded)
  );
  assert_eq!(db.runs_of_task(first.task()).expect("runs").len(), 1);
  assert_eq!(
    db.run_with_key(first.task().first_run_key())
      .expect("first")
      .expect("exists")
      .id(),
    first.id()
  );
  assert!(
    db.run_with_key(group.tasks()[1].first_run_key())
      .expect("second")
      .is_none()
  );
  fresh.stop_all(InterruptCause::QuitNow).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn task_group_fresh_pending_first_recovery_reuses_id_key_snapshot_and_history() {
  let fixture = project(&Setup::new("Append one line."));
  let root = fixture.repo.path();
  let mut db = ProjectDb::open(root).expect("db");
  let group = reviewed_group(&mut db, fixture.task, Scheduling::Sequential, 3_000_000);
  let slots = NonZeroU32::new(2).expect("slots");
  let (original, mut events) = scheduler(root);
  original.attach(AttachSession::Reconnect, slots).await.expect("attach");
  original.advance_task_groups().await.expect("advance");
  let first = db
    .run_with_key(group.tasks()[0].first_run_key())
    .expect("first")
    .expect("exists");
  let succeeded = wait_for(root, &mut events, first.id(), |run| {
    run.status() == &RunStatus::Succeeded
  })
  .await;
  let pending = record_pending_member(&mut db, &group, &first);
  let task = pending.task();
  original.stop_all(InterruptCause::QuitNow).await;
  drop(original);
  drop(db);
  let db = ProjectDb::open(root).expect("reopened");
  let (fresh, _) = reopen(root, slots).await;
  fresh.advance_task_groups().await.expect("still held");
  assert_eq!(db.run(pending.id()).expect("run").expect("exists"), pending);
  assert_eq!(
    db.task_group_hold(group.key()).expect("hold"),
    Some(GroupHold::SessionEnded)
  );
  fresh.stop_all(InterruptCause::QuitNow).await;
  drop(fresh);
  // The Pending first run's start had failed before the project closed: reopening does not retry
  // it on its own either.
  db.set_task_group_hold(group.key(), Some(&GroupHold::StartFailed))
    .expect("failed start");
  let (reopened, mut events) = reopen(root, slots).await;
  reopened.advance_task_groups().await.expect("still held");
  assert_eq!(db.run(pending.id()).expect("run").expect("exists"), pending);
  assert_eq!(
    db.task_group_hold(group.key()).expect("hold"),
    Some(GroupHold::SessionEnded)
  );
  let recovered = reopened
    .start_requested_run(task.first_run_key(), task)
    .await
    .expect("recover existing first");
  assert_eq!(recovered, pending.id());
  assert_eq!(
    reopened
      .start_requested_run(task.first_run_key(), task)
      .await
      .expect("lost-reply retry"),
    recovered
  );
  let finished = wait_for(root, &mut events, recovered, |run| {
    run.status() == &RunStatus::Succeeded
  })
  .await;
  assert_recovered_first(&db, &pending, &finished);
  assert_eq!(db.run(first.id()).expect("earlier").expect("exists"), succeeded);
  assert_eq!(db.task_group(group.key()).expect("group").expect("exists"), group);
  assert_ne!(
    db.task_group_hold(group.key()).expect("hold"),
    Some(GroupHold::SessionEnded)
  );
}

/// A runner of the project reopened after Quit now, attached with `slots`.
async fn reopen(
  root: &std::path::Path,
  slots: NonZeroU32,
) -> (
  orchester_runner::execution::Scheduler,
  tokio::sync::mpsc::UnboundedReceiver<orchester_ipc::Event>,
) {
  let (reopened, events) = scheduler(root);
  reopened
    .attach(
      AttachSession::Fresh {
        previous_end: orchester_ipc::SessionEnd::QuitNow,
      },
      slots,
    )
    .await
    .expect("fresh attach");
  (reopened, events)
}

fn record_pending_member(
  db: &mut ProjectDb,
  group: &TaskGroup,
  first: &orchester_core::run::Run,
) -> orchester_core::run::Run {
  let task = db.task(group.tasks()[1]).expect("task").expect("exists");
  let limits = task.details().limits;
  db.create_run(
    task.id().first_run_key(),
    orchester_core::run::NewRun {
      task: task.id(),
      snapshot: first.snapshot().clone(),
      branch: orchester_core::run::BranchName::for_next_run(task.key(), 0),
      base: first.base().clone(),
      base_branch: None,
      budget: limits.budget(),
      max_duration: limits.max_duration(),
    },
    Timestamp::now(),
  )
  .expect("record first run without starting")
}

fn assert_recovered_first(db: &ProjectDb, pending: &orchester_core::run::Run, finished: &orchester_core::run::Run) {
  assert_eq!(finished.snapshot(), pending.snapshot());
  assert_eq!(finished.branch(), pending.branch());
  assert_eq!(
    db.run_with_key(pending.task().first_run_key())
      .expect("first key")
      .expect("exists")
      .id(),
    pending.id()
  );
  assert_eq!(db.runs_of_task(pending.task()).expect("runs").len(), 1);
}
