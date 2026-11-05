//! A local task's organization through the dispatcher: the person's changes go over what is stored now, and closing a
//! task whose run has not ended is refused and changes nothing.

use orchester_core::{
  id::{IdempotencyKey, TaskId},
  store::TaskStore,
  task::organization::{ChangeActor, OrganizationError, Priority, TaskOrganization, WorkflowStatus},
  time::Timestamp,
};
use orchester_store::StoreError;
use orchester_workspace::actions::{Action, ActionError, ActionOutcome, Intent, LocalResult, TaskAction};

use crate::{Checkout, details, run::start_run};

/// A local task and its organization as stored.
fn task(checkout: &Checkout) -> (TaskId, TaskOrganization) {
  let mut db = checkout.db();
  let task = db
    .create_tasks(IdempotencyKey::generate(), vec![details("Sign in")], Timestamp::now())
    .expect("a local task")
    .remove(0)
    .id();
  (task, db.task_organization(task).expect("organization"))
}

fn organize(checkout: &Checkout, task: TaskId, base: &TaskOrganization, edited: TaskOrganization) -> Intent {
  let read = checkout.db().task(task).expect("read").expect("the task").revision();
  Intent::new(Action::Task(TaskAction::Organize {
    task,
    read,
    base: base.clone(),
    edited,
  }))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn actions_a_local_organize_keeps_a_change_made_meanwhile() {
  let checkout = Checkout::local();
  let (task, base) = task(&checkout);
  let intent = organize(&checkout, task, &base, base.clone().with_priority(Priority::High));
  checkout
    .db()
    .update_task_organization(task, ChangeActor::Orchester, Timestamp::now(), |value| {
      value.with_status(WorkflowStatus::InProgress)
    })
    .expect("the Orchestrator moves it");
  let outcome = checkout.local_dispatcher().dispatch(&intent).await.expect("dispatched");
  assert!(
    matches!(outcome, ActionOutcome::Local(LocalResult::Changed(true))),
    "{outcome:?}"
  );
  let stored = checkout.db().task_organization(task).expect("organization");
  assert_eq!(
    (stored.status(), stored.priority()),
    (WorkflowStatus::InProgress, Priority::High)
  );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn actions_a_local_close_with_an_active_run_changes_nothing() {
  let checkout = Checkout::local();
  let (task, _) = task(&checkout);
  start_run(&mut checkout.db(), task);
  let base = checkout.db().task_organization(task).expect("organization");
  let dispatcher = checkout.local_dispatcher();
  for status in [WorkflowStatus::Done, WorkflowStatus::Canceled] {
    let edited = base.clone().with_status(status).with_priority(Priority::High);
    let refused = dispatcher.dispatch(&organize(&checkout, task, &base, edited)).await;
    assert!(
      matches!(&refused, Err(ActionError::Write(error)) if matches!(
        error.store(),
        Some(StoreError::Organization(OrganizationError::StopRunFirst(_)))
      )),
      "{refused:?}"
    );
    assert_eq!(checkout.db().task_organization(task).expect("organization"), base);
  }

  let (open, base) = self::task(&checkout);
  let edited = base
    .clone()
    .with_status(WorkflowStatus::Done)
    .with_priority(Priority::High);
  dispatcher
    .dispatch(&organize(&checkout, open, &base, edited.clone()))
    .await
    .expect("closed without a run");
  assert_eq!(checkout.db().task_organization(open).expect("organization"), edited);
}
