//! A task set to Canceled while its group waits is skipped: the group never starts it, its status
//! stays Canceled, and the group goes on with the next task and finishes.
use std::num::NonZeroU32;

use orchester_core::{
  run::RunStatus,
  store::RunStore,
  task::organization::{ChangeActor, TaskOrganization, WorkflowStatus},
  task_group::Scheduling,
  time::Timestamp,
};
use orchester_ipc::AttachSession;
use orchester_store::ProjectDb;

use super::{
  reviewed_group,
  support::{Setup, project, scheduler, wait_for},
};

#[tokio::test(flavor = "multi_thread")]
async fn task_group_skips_a_canceled_task_and_goes_on() {
  let fixture = project(&Setup::new("Append one line."));
  let root = fixture.repo.path();
  let mut db = ProjectDb::open(root).expect("db");
  let group = reviewed_group(&mut db, fixture.task, Scheduling::Sequential, 3_000_000);
  let (canceled, next) = (group.tasks()[0], group.tasks()[1]);
  let value = TaskOrganization::default().with_status(WorkflowStatus::Canceled);
  db.set_task_organization(canceled, &value, ChangeActor::Person, Timestamp::now())
    .expect("cancel the first task");
  let (scheduler, mut events) = scheduler(root);
  scheduler
    .attach(AttachSession::Reconnect, NonZeroU32::new(4).expect("slots"))
    .await
    .expect("attach");
  scheduler.advance_task_groups().await.expect("advance");
  assert!(db.run_with_key(canceled.first_run_key()).expect("read").is_none());
  let run = db
    .run_with_key(next.first_run_key())
    .expect("read")
    .expect("the task after the canceled one starts in its turn");
  wait_for(root, &mut events, run.id(), |run| run.status() == &RunStatus::Succeeded).await;
  scheduler.advance_task_groups().await.expect("advance");
  assert!(db.run_with_key(canceled.first_run_key()).expect("read").is_none());
  assert!(db.unfinished_task_groups().expect("groups").is_empty());
  assert_eq!(
    db.task_organization(canceled).expect("status").status(),
    WorkflowStatus::Canceled
  );
}
