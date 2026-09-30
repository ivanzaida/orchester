mod support;

use orchester_core::{
  id::IdempotencyKey,
  run::{InterruptCause, RunAgainBlock},
  store::{RunStore, TaskStore},
  task::organization::{BlockingTask, ChangeActor, DependencyGate, Priority, TaskOrganization, WorkflowStatus},
  time::Timestamp,
};
use orchester_ipc::Rejection;
use orchester_store::ProjectDb;
use support::{Setup, project, scheduler, wait_for_turns};

#[tokio::test(flavor = "multi_thread")]
async fn organization_runner_blocks_unfinished_links_and_keeps_manual_reason() {
  let project = project(&Setup::new("Keep working."));
  let root = project.repo.path();
  let mut db = ProjectDb::open(root).expect("open");
  let details = db.task(project.task).expect("read").expect("task").details().clone();
  let blocker = db
    .create_tasks(IdempotencyKey::generate(), vec![details], Timestamp::now())
    .expect("blocker")[0]
    .id();
  let linked = TaskOrganization::new(
    WorkflowStatus::Todo,
    Priority::Urgent,
    Some("Needs integration".into()),
    vec![BlockingTask {
      task: blocker,
      gate: DependencyGate::Merge,
    }],
    vec![],
  )
  .expect("metadata");
  db.set_task_organization(project.task, &linked, ChangeActor::Person, Timestamp::now())
    .expect("link");
  let (scheduler, mut events) = scheduler(root);
  assert_eq!(
    scheduler.start_run(IdempotencyKey::generate(), project.task).await,
    Err(Rejection::RunAgainBlocked {
      block: RunAgainBlock::TaskBlocked
    })
  );
  assert!(db.runs_of_task(project.task).expect("runs").is_empty());
  let informational = TaskOrganization::new(
    WorkflowStatus::Todo,
    Priority::Urgent,
    Some("Needs integration".into()),
    vec![],
    vec![],
  )
  .expect("metadata");
  db.set_task_organization(project.task, &informational, ChangeActor::Person, Timestamp::now())
    .expect("unlink");
  let run = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("informational flag permits run");
  wait_for_turns(root, &mut events, run, 1).await;
  assert_eq!(
    db.task_organization(project.task).expect("metadata").status(),
    WorkflowStatus::InProgress
  );
  scheduler.cancel_run(run).await.expect("existing cancel path");
  db.set_task_organization(
    project.task,
    &informational.with_status(WorkflowStatus::Canceled),
    ChangeActor::Person,
    Timestamp::now(),
  )
  .expect("status after stop");
  assert!(db.run(run).expect("read").expect("run").status().is_terminal());
  scheduler.stop_all(InterruptCause::QuitNow).await;
}
