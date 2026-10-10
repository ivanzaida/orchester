//! Organization changes that meet a change made elsewhere meanwhile: Edit task keeps the status
//! Orchester or another view set while it was open, and a confirmed bulk Canceled does not stop a
//! run that started after the confirmation opened.

use std::{path::Path, time::Duration};

use orchester_core::{
  id::IdempotencyKey,
  name::Name,
  resource::ResourceKind,
  store::{ResourceStore, RunStore, TaskStore},
  task::{
    TaskDetails, TaskLimits,
    organization::{ChangeActor, WorkflowStatus},
  },
  tokens::TokenBudget,
};
use orchester_store::ProjectDb;

use crate::{
  harness::Desktop,
  organization::tasks,
  task_run::set_text,
  unreadable_run::{at, delivery, started},
};

const STOPS: usize = 60;

fn details(pipeline: orchester_core::id::ResourceId, title: &str) -> TaskDetails {
  TaskDetails {
    title: Name::new(title).expect("title"),
    brief: "Implement.".into(),
    acceptance_criteria: vec!["Complete.".into()],
    pipeline: Some(pipeline),
    limits: TaskLimits::new(
      Some(TokenBudget::new(10_000).expect("budget")),
      Some(Duration::from_secs(1_800)),
    )
    .expect("limits"),
  }
}

/// KON-1 with run #1 started and not ended; KON-2 without a run.
fn seed(repo: &Path) {
  let mut db = ProjectDb::open(repo).expect("open");
  let snapshot = delivery(&mut db);
  let pipeline = db.resources(ResourceKind::Pipeline).expect("pipelines")[0].id();
  let created = db
    .create_tasks(
      IdempotencyKey::generate(),
      vec![details(pipeline, "Working"), details(pipeline, "Idle")],
      at(10),
    )
    .expect("tasks");
  started(&mut db, (created[0].id(), &snapshot), 1, 11);
}

/// M1: Edit task opened while the task was In progress; it moved to In review meanwhile. Saving a
/// new title keeps In review and adds no history row for a status nobody changed.
#[test]
fn organization_edit_keeps_a_status_set_while_the_form_was_open() {
  let mut desktop = Desktop::with_seeded_project(seed);
  desktop.wait_for("nav-tasks");
  desktop.click("nav-tasks");
  desktop.wait_for("task-row-KON-1");
  desktop.press("task-row-KON-1");
  desktop.wait_for("task-edit");
  desktop.click("task-edit");
  desktop.wait_until("the form filled with the task", |desktop| {
    desktop.value("task-title") == "Working"
  });
  let task = tasks(&desktop.repo)[0].id();
  let mut db = ProjectDb::open(&desktop.repo).expect("open");
  let review = db
    .task_organization(task)
    .expect("read")
    .with_status(WorkflowStatus::InReview);
  db.set_task_organization(task, &review, ChangeActor::Orchester, at(50))
    .expect("in review meanwhile");
  let rows = db.task_organization_history(task).expect("history").len();
  set_text(&mut desktop, "task-title", "Working, renamed");
  desktop.wait_until("Save changes available", |desktop| {
    desktop.stops(STOPS, false).contains(&Some("save-draft".into()))
  });
  desktop.tab_to("save-draft", STOPS);
  desktop.enter();
  desktop.wait_until("the new title saved", |desktop| {
    tasks(&desktop.repo)[0].details().title.as_str() == "Working, renamed"
  });
  let db = ProjectDb::open(&desktop.repo).expect("open");
  assert_eq!(
    db.task_organization(task).expect("read").status(),
    WorkflowStatus::InReview
  );
  assert_eq!(db.task_organization_history(task).expect("history").len(), rows);
}

/// M2: the bulk confirmation named KON-1's run only; a run of KON-2 that starts before Stop is
/// pressed keeps running, KON-2 is not canceled, and the outcome names it.
#[test]
fn organization_bulk_cancel_does_not_stop_a_run_started_after_it_opened() {
  let mut desktop = Desktop::with_seeded_project(seed);
  desktop.wait_for("nav-tasks");
  desktop.click("nav-tasks");
  desktop.wait_for("task-row-KON-2");
  desktop.press("task-select-KON-1");
  desktop.press("task-select-KON-2");
  desktop.press("bulk-status");
  desktop.wait_for("bulk-status-Canceled");
  desktop.press("bulk-status-Canceled");
  desktop.wait_for("bulk-cancel-stop");
  let ids: Vec<_> = tasks(&desktop.repo).iter().map(|task| task.id()).collect();
  let mut db = ProjectDb::open(&desktop.repo).expect("open");
  let snapshot = db.runs_of_task(ids[0]).expect("runs")[0]
    .as_ref()
    .expect("a readable run")
    .snapshot()
    .clone();
  let late = started(&mut db, (ids[1], &snapshot), 2, 60);
  desktop.tab_to("bulk-cancel-stop", 8);
  desktop.enter();
  desktop.wait_until("the outcome names KON-2", |desktop| {
    crate::agent_permissions::shows(
      desktop,
      "KON-2 could not be updated: A run started after the confirmation opened",
    )
  });
  let db = ProjectDb::open(&desktop.repo).expect("open");
  assert!(
    !db.run(late.id()).expect("read").expect("run").status().is_terminal(),
    "the late run keeps running"
  );
  assert_ne!(
    db.task_organization(ids[1]).expect("read").status(),
    WorkflowStatus::Canceled
  );
}
