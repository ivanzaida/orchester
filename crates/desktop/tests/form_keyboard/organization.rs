use std::path::Path;

use orchester_core::{
  id::IdempotencyKey,
  name::Name,
  store::TaskStore,
  task::{
    TaskDetails,
    organization::{ChangeActor, Priority, TaskOrganization, WorkflowStatus},
  },
  time::Timestamp,
};
use orchester_store::ProjectDb;

use crate::{harness::Desktop, task_run::set_text};

fn choose(desktop: &mut Desktop, id: &str, label: &str) {
  let mut select = desktop
    .tree
    .get_element_by_id_mut(id)
    .and_then(|element| element.as_select())
    .expect("select");
  let index = select
    .labels()
    .iter()
    .position(|option| option.as_ref() == label)
    .expect("option");
  select.commit(index);
  desktop.pass();
}
fn tasks(repo: &Path) -> Vec<orchester_core::task::Task> {
  ProjectDb::open(repo).expect("open").tasks().expect("tasks")
}

#[test]
fn organization_new_form_saves_status_priority_informational_reason_and_inline_tag() {
  let mut desktop = Desktop::with_project();
  desktop.wait_for("nav-tasks");
  desktop.click("nav-tasks");
  desktop.wait_for("tasks-new-task");
  desktop.click("tasks-new-task");
  desktop.wait_for("task-title");
  set_text(&mut desktop, "task-title", "Organized task");
  choose(&mut desktop, "task-workflow-field", "Todo");
  choose(&mut desktop, "task-priority-field", "High");
  desktop.press("task-blocked-field");
  desktop.wait_for("task-blocked-reason");
  set_text(&mut desktop, "task-blocked-reason", "Waiting for a decision");
  desktop.press("task-tags-picker");
  desktop.wait_for("tag-search");
  set_text(&mut desktop, "tag-search", "Desktop");
  desktop.tab_to("tag-create-inline", 40);
  desktop.enter();
  desktop.wait_until("inline tag creation", |desktop| {
    ProjectDb::open(&desktop.repo)
      .expect("open")
      .project_tags()
      .expect("tags")
      .len()
      == 1
  });
  desktop.tab_to("save-draft", 40);
  desktop.enter();
  desktop.wait_until("organization saved", |desktop| {
    tasks(&desktop.repo)
      .first()
      .is_some_and(|task| task.organization().tags().len() == 1)
  });
  let task = tasks(&desktop.repo).remove(0);
  assert_eq!(task.organization().status(), WorkflowStatus::Todo);
  assert_eq!(task.organization().priority(), Priority::High);
  assert_eq!(task.organization().blocked_reason(), Some("Waiting for a decision"));
  ProjectDb::open(&desktop.repo)
    .expect("open")
    .check_task_readiness(task.id())
    .expect("informational flag permits run");
}

#[test]
fn organization_bulk_status_priority_and_filters() {
  let mut desktop = Desktop::with_seeded_project(|repo| {
    let mut db = ProjectDb::open(repo).expect("open");
    let details = TaskDetails {
      title: Name::new("First").expect("title"),
      brief: String::new(),
      acceptance_criteria: vec![],
      pipeline: None,
      limits: None,
    };
    db.create_tasks(
      IdempotencyKey::generate(),
      vec![details.clone(), details],
      Timestamp::now(),
    )
    .expect("tasks");
  });
  desktop.wait_for("nav-tasks");
  desktop.click("nav-tasks");
  desktop.wait_for("task-row-KON-1");
  let ids: Vec<_> = tasks(&desktop.repo).iter().map(|task| task.id()).collect();
  for id in &ids {
    desktop.press(&format!("select-task-{id}"));
  }
  desktop.press("bulk-status");
  desktop.wait_for("bulk-status-InReview");
  desktop.press("bulk-status-InReview");
  desktop.wait_until("bulk status", |desktop| {
    tasks(&desktop.repo)
      .iter()
      .all(|task| task.organization().status() == WorkflowStatus::InReview)
  });
  desktop.wait_for(&format!("bulk-outcome-{}", ids[0]));
  desktop.press("bulk-priority");
  desktop.wait_for("bulk-priority-Urgent");
  desktop.press("bulk-priority-Urgent");
  desktop.wait_until("bulk priority", |desktop| {
    tasks(&desktop.repo)
      .iter()
      .all(|task| task.organization().priority() == Priority::Urgent)
  });
  desktop.press("filter-status");
  desktop.wait_for("filter-status-Todo");
  desktop.press("filter-status-Todo");
  assert!(desktop.tree.get_element_by_id("task-row-KON-1").is_none());
  desktop.press("filters-clear");
  desktop.wait_for("task-row-KON-1");
}

#[test]
fn organization_tag_delete_confirmation_escape_preserves_assignments() {
  let mut desktop = Desktop::with_seeded_project(|repo| {
    let mut db = ProjectDb::open(repo).expect("open");
    let details = TaskDetails {
      title: Name::new("Tagged").expect("title"),
      brief: String::new(),
      acceptance_criteria: vec![],
      pipeline: None,
      limits: None,
    };
    let task = db
      .create_tasks(IdempotencyKey::generate(), vec![details], Timestamp::now())
      .expect("task")[0]
      .id();
    let tag = db.create_project_tag(Name::new("Desktop").expect("name")).expect("tag");
    db.set_task_organization(
      task,
      &TaskOrganization::default().with_tags(vec![tag.id()]),
      ChangeActor::Person,
      Timestamp::now(),
    )
    .expect("tagging");
  });
  desktop.wait_for("nav-tasks");
  desktop.click("nav-tasks");
  desktop.wait_for("project-tags-manage");
  desktop.press("project-tags-manage");
  let tag = ProjectDb::open(&desktop.repo)
    .expect("open")
    .project_tags()
    .expect("tags")[0]
    .id();
  let delete = format!("project-tag-delete-{tag}");
  desktop.wait_for(&delete);
  desktop.press(&delete);
  desktop.wait_for("tag-editor-confirm");
  desktop.key("Escape", "Escape");
  assert_eq!(tasks(&desktop.repo)[0].organization().tags(), &[tag]);
  desktop.press(&delete);
  desktop.wait_for("tag-editor-confirm");
  desktop.tab_to("tag-editor-confirm", 8);
  desktop.enter();
  desktop.wait_until("tag deletion removes assignments", |desktop| {
    tasks(&desktop.repo)[0].organization().tags().is_empty()
  });
  assert!(
    ProjectDb::open(&desktop.repo)
      .expect("open")
      .project_tags()
      .expect("tags")
      .is_empty()
  );
}

#[test]
fn organization_cancel_active_run_confirms_and_preserves_branch() {
  use crate::task_run::{create_and_run, init_repository, latest_run, programs, seed};
  let (runner, fake) = programs();
  let mut desktop = Desktop::with_runner(runner, init_repository, |repo| seed(repo, &fake, false));
  create_and_run(&mut desktop, "Keep working.", "1400000");
  desktop.wait_until("active run", |desktop| {
    latest_run(&desktop.repo).is_some_and(|run| !run.status().is_terminal())
  });
  let run = latest_run(&desktop.repo).expect("run");
  desktop.wait_for("task-header-status");
  choose(&mut desktop, "task-header-status", "Canceled");
  desktop.wait_for("task-cancel-confirmation");
  assert_eq!(
    tasks(&desktop.repo)[0].organization().status(),
    WorkflowStatus::InProgress
  );
  desktop.key("Escape", "Escape");
  assert!(desktop.tree.get_element_by_id("task-cancel-confirmation").is_none());
  assert!(!latest_run(&desktop.repo).expect("run").status().is_terminal());
  choose(&mut desktop, "task-header-status", "Canceled");
  desktop.wait_for("task-cancel-stop");
  desktop.tab_to("task-cancel-stop", 5);
  desktop.enter();
  desktop.wait_until("task canceled after stop", |desktop| {
    tasks(&desktop.repo)[0].organization().status() == WorkflowStatus::Canceled
  });
  let stopped = latest_run(&desktop.repo).expect("run");
  assert!(stopped.status().is_terminal());
  assert_eq!(stopped.branch(), run.branch());
  assert!(
    desktop
      .repo
      .join(".orchester/worktrees")
      .join(run.id().to_string())
      .is_dir()
  );
}
