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

#[tokio::test(flavor = "multi_thread")]
async fn organization_follow_up_and_recovery_reject_blockers_without_changing_work() {
  for brief in ["Build it.", "Report failure."] {
    check_restart(brief).await;
  }
}

async fn check_restart(brief: &'static str) {
  use std::fs;

  use orchester_core::{run::RunStatus, store::ConversationStore};
  use support::{wait_for, wait_for_idle};
  let project = project(&Setup::new(brief));
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("start");
  let ended = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  wait_for_idle(&scheduler).await;
  let path = root.join(".orchester/worktrees").join(id.to_string());
  fs::write(path.join("keep-uncommitted.txt"), "keep me").expect("work");
  let mut db = ProjectDb::open(root).expect("open");
  let linked = link_blocker(&mut db, project.task);
  let original_events = db.events(id).expect("events");
  let original_history = db.task_organization_history(project.task).expect("history");
  let messages = queue_restart(&mut db, &ended);
  assert_eq!(
    scheduler.deliver_messages(id).await,
    Err(Rejection::RunAgainBlocked {
      block: RunAgainBlock::TaskBlocked
    })
  );
  assert_eq!(db.run(id).expect("read").expect("run"), ended);
  assert_eq!(db.events(id).expect("events"), original_events);
  assert_eq!(
    db.task_organization_history(project.task).expect("history"),
    original_history
  );
  assert_eq!(db.messages(project.task).expect("messages"), messages);
  assert_eq!(
    fs::read_to_string(path.join("keep-uncommitted.txt")).expect("work"),
    "keep me"
  );
  let informational = TaskOrganization::new(
    linked.status(),
    linked.priority(),
    linked.blocked_reason().map(str::to_owned),
    vec![],
    vec![],
  )
  .expect("organization");
  db.set_task_organization(project.task, &informational, ChangeActor::Person, Timestamp::now())
    .expect("unlink");
  scheduler
    .deliver_messages(id)
    .await
    .expect("informational flag permits restart");
  let done = wait_for(root, &mut events, id, |run| {
    run.attempts().len() == 2 && run.status().is_terminal()
  })
  .await;
  wait_for_idle(&scheduler).await;
  assert_eq!(done.status(), &RunStatus::Succeeded);
  assert_eq!(done.branch(), ended.branch());
  let history = db.task_organization_history(project.task).expect("history");
  assert!(history.iter().any(|change| change.actor == ChangeActor::Orchester
    && change.from.status() == WorkflowStatus::Done
    && change.to.status() == WorkflowStatus::InProgress));
  let value = db.task_organization(project.task).expect("organization");
  assert_eq!(value.priority(), Priority::Urgent);
  assert_eq!(value.blocked_reason(), Some("Needs integration"));
  scheduler.stop_all(InterruptCause::QuitNow).await;
}

fn link_blocker(db: &mut ProjectDb, task: orchester_core::id::TaskId) -> TaskOrganization {
  let details = db.task(task).expect("read").expect("task").details().clone();
  let blocker = db
    .create_tasks(IdempotencyKey::generate(), vec![details], Timestamp::now())
    .expect("blocker")[0]
    .id();
  let value = TaskOrganization::new(
    WorkflowStatus::Done,
    Priority::Urgent,
    Some("Needs integration".into()),
    vec![BlockingTask {
      task: blocker,
      gate: DependencyGate::Merge,
    }],
    vec![],
  )
  .expect("organization");
  db.set_task_organization(task, &value, ChangeActor::Person, Timestamp::now())
    .expect("link");
  value
}

fn queue_restart(db: &mut ProjectDb, ended: &orchester_core::run::Run) -> Vec<orchester_core::conversation::Message> {
  use orchester_core::{
    conversation::{Message, MessageBody, Recipient},
    run::RunStatus,
    store::ConversationStore,
  };
  let message = Message::from_person(
    ended.task(),
    Some(ended.id()),
    Recipient::Orchestrator,
    MessageBody::new("Continue with this correction.").expect("body"),
    Timestamp::now(),
  );
  let message = if matches!(ended.status(), RunStatus::Failed(_)) {
    message
      .recovering_after(ended.last_sequence())
      .expect("recovery message")
  } else {
    message
  };
  db.send_message(IdempotencyKey::generate(), &message)
    .expect("message queued");
  db.messages(ended.task()).expect("messages")
}
