//! Task organization after a change: a failed save does not pass for the last one that
//! succeeded, the task header's pickers follow the task as it is now, the locked Retry form keeps
//! the stored organization, and a tag picker that cannot be used creates no tag.

use std::{path::Path, time::Duration};

use orchester_core::{
  id::{IdempotencyKey, ResourceId},
  name::Name,
  resource::ResourceKind,
  store::{ResourceStore, TaskStore},
  task::{
    TaskDetails, TaskLimits,
    organization::{ChangeActor, Priority, TaskOrganization, WorkflowStatus},
  },
  tokens::TokenBudget,
};
use orchester_store::ProjectDb;

use crate::{
  harness::Desktop,
  organization::{choose, tasks},
  task_run::{init_repository, programs, seed, set_text, unborn_head},
  unreadable_run::{at, delivery, started},
};

/// Tab stops enough to pass every control of the task page and the form.
const STOPS: usize = 60;

fn complete(pipeline: ResourceId) -> TaskDetails {
  TaskDetails {
    title: Name::new("Organized").expect("title"),
    brief: "Implement.".into(),
    acceptance_criteria: vec!["Complete.".into()],
    pipeline: Some(pipeline),
    limits: Some(
      TaskLimits::new(TokenBudget::new(10_000).expect("budget"), Duration::from_secs(1_800)).expect("limits"),
    ),
  }
}

/// KON-1, complete, with run #1 started and not ended.
fn seed_active(repo: &Path) {
  let mut db = ProjectDb::open(repo).expect("open");
  let snapshot = delivery(&mut db);
  let pipeline = db.resources(ResourceKind::Pipeline).expect("pipelines")[0].id();
  let task = db
    .create_tasks(IdempotencyKey::generate(), vec![complete(pipeline)], at(10))
    .expect("task")[0]
    .id();
  started(&mut db, (task, &snapshot), 1, 11);
}

fn open_task(desktop: &mut Desktop) {
  desktop.wait_for("nav-tasks");
  desktop.click("nav-tasks");
  desktop.wait_for("task-row-KON-1");
  desktop.press("task-row-KON-1");
  desktop.wait_for("task-header-status");
}

fn stored_status(desktop: &Desktop) -> WorkflowStatus {
  tasks(&desktop.repo)[0].organization().status()
}

fn stored_priority(desktop: &Desktop) -> Priority {
  tasks(&desktop.repo)[0].organization().priority()
}

fn reachable(desktop: &mut Desktop, id: &str) -> bool {
  desktop.stops(STOPS, false).contains(&Some(id.into()))
}

/// Presses Save changes once the form can be saved: not while an earlier save is under way.
fn save_changes(desktop: &mut Desktop) {
  desktop.wait_until("Save changes available", |desktop| reachable(desktop, "save-draft"));
  desktop.tab_to("save-draft", STOPS);
  desktop.enter();
}

/// Chooses `label` in the header picker `id` and waits until the header saved it: its tag
/// picker, the header's one tab stop, can be used again, and the picker shows `label`. A read of
/// the task that began before the save can still arrive and show the earlier value for a moment;
/// choosing what the picker already shows would save nothing.
fn choose_in_header(desktop: &mut Desktop, id: &str, label: &str, saved: impl Fn(&Desktop) -> bool) {
  choose(desktop, id, label);
  desktop.wait_until(label, |desktop| {
    saved(desktop) && reachable(desktop, "task-tags-picker") && picks(desktop, id, label)
  });
}

/// Whether the picker `id` shows `label`, and only it, as chosen.
fn picks(desktop: &mut Desktop, id: &str, label: &str) -> bool {
  desktop
    .tree
    .get_element_by_id_mut(id)
    .and_then(|element| element.as_select())
    .is_some_and(|select| select.selected_labels().iter().map(AsRef::as_ref).eq([label]))
}

/// R1: after one successful save, Canceled for a task with an active run keeps the confirmation
/// open instead of taking the earlier save's data for its own success.
#[test]
fn organization_edit_form_keeps_the_cancel_confirmation_after_an_earlier_save() {
  let mut desktop = Desktop::with_seeded_project(seed_active);
  open_task(&mut desktop);
  desktop.wait_for("task-edit");
  desktop.click("task-edit");
  desktop.wait_for("task-workflow-field");
  desktop.wait_until("the form filled with the task", |desktop| {
    desktop.value("task-title") == "Organized"
  });
  choose(&mut desktop, "task-priority-field", "High");
  save_changes(&mut desktop);
  desktop.wait_until("the first save", |desktop| stored_priority(desktop) == Priority::High);
  choose(&mut desktop, "task-workflow-field", "Canceled");
  save_changes(&mut desktop);
  desktop.wait_for("edit-task-cancel-confirmation");
  for _ in 0..5 {
    desktop.pass();
  }
  assert!(
    desktop
      .tree
      .get_element_by_id("edit-task-cancel-confirmation")
      .is_some(),
    "the confirmation stays open until it is answered"
  );
  // N2: asking to confirm is not a failed save, in the status bar or under the actions.
  assert!(!crate::agent_permissions::shows(&mut desktop, "Could not save task"));
  assert!(!crate::agent_permissions::shows(
    &mut desktop,
    "Confirm stopping the active run"
  ));
  assert_ne!(stored_status(&desktop), WorkflowStatus::Canceled);
}

/// R1: stopping the run fails without a runner; the confirmation stays open with the failure
/// although an earlier change of the header succeeded.
#[test]
fn organization_header_keeps_a_failed_cancellation_open() {
  let mut desktop = Desktop::with_seeded_project(seed_active);
  open_task(&mut desktop);
  choose_in_header(&mut desktop, "task-header-priority", "High", |desktop| {
    stored_priority(desktop) == Priority::High
  });
  choose(&mut desktop, "task-header-status", "Canceled");
  desktop.wait_for("task-cancel-stop");
  desktop.tab_to("task-cancel-stop", 5);
  desktop.enter();
  desktop.wait_until("the failed stop", |desktop| reachable(desktop, "task-cancel-stop"));
  assert!(desktop.tree.get_element_by_id("task-cancel-confirmation").is_some());
  assert_ne!(stored_status(&desktop), WorkflowStatus::Canceled);
}

/// R1: a failed rename after a successful creation keeps its editor open with the failure.
#[test]
fn organization_tag_manager_keeps_the_editor_of_a_failed_change() {
  let mut desktop = Desktop::with_project();
  desktop.wait_for("nav-tasks");
  desktop.click("nav-tasks");
  desktop.wait_for("project-tags-manage");
  desktop.press("project-tags-manage");
  desktop.wait_for("project-tag-new");
  desktop.press("project-tag-new");
  desktop.wait_for("project-tag-name");
  set_text(&mut desktop, "project-tag-name", "Desktop");
  desktop.tab_to("tag-editor-confirm", 12);
  desktop.enter();
  desktop.wait_until("the created tag", |desktop| {
    desktop.tree.get_element_by_id("project-tag-name").is_none()
  });
  let tag = ProjectDb::open(&desktop.repo)
    .expect("open")
    .project_tags()
    .expect("tags")[0]
    .id();
  desktop.press(&format!("project-tag-rename-{tag}"));
  desktop.wait_for("project-tag-name");
  set_text(&mut desktop, "project-tag-name", "Client");
  // Held past the store's busy timeout, so the rename fails.
  let connection = rusqlite::Connection::open(desktop.repo.join(".orchester/orchester.db")).expect("connection");
  connection
    .execute_batch("BEGIN IMMEDIATE")
    .expect("hold the write lock");
  desktop.tab_to("tag-editor-confirm", 12);
  desktop.enter();
  desktop.wait_until("the failed rename", |desktop| {
    desktop.tree.get_element_by_id("project-tag-name").is_none() || reachable(desktop, "tag-editor-confirm")
  });
  connection.execute_batch("ROLLBACK").expect("release the write lock");
  assert!(
    desktop.tree.get_element_by_id("project-tag-name").is_some(),
    "the editor stays open with its error"
  );
  let names: Vec<String> = ProjectDb::open(&desktop.repo)
    .expect("open")
    .project_tags()
    .expect("tags")
    .iter()
    .map(|tag| tag.name().as_str().to_owned())
    .collect();
  assert_eq!(names, ["Desktop"]);
}

/// R2: the header compares a choice with the task as it is now. Once its run has ended, Canceled
/// saves without asking, and no observer left from a render while the run was active reopens the
/// confirmation or puts the picker back.
#[test]
fn organization_header_follows_the_run_ending() {
  use crate::task_run::{create_and_run, init_repository, latest_run, programs, seed};
  let (runner, fake) = programs();
  let mut desktop = Desktop::with_runner(runner, init_repository, |repo| seed(repo, &fake, false));
  create_and_run(&mut desktop, "Keep working.", "1400000");
  desktop.wait_until("an active run", |desktop| {
    latest_run(&desktop.repo).is_some_and(|run| !run.status().is_terminal())
  });
  desktop.wait_for("task-header-status");
  choose(&mut desktop, "task-header-status", "Canceled");
  desktop.wait_for("task-cancel-stop");
  desktop.tab_to("task-cancel-stop", 5);
  desktop.enter();
  desktop.wait_until("the run stopped and the task canceled", |desktop| {
    stored_status(desktop) == WorkflowStatus::Canceled
      && desktop.tree.get_element_by_id("task-cancel-confirmation").is_none()
      && crate::github::shows(desktop, "task-page", "Canceled")
      // The page shows the ended run, so no earlier read of the task is still on its way.
      && crate::github::shows(desktop, "task-page", "Canceled run")
  });
  choose_in_header(&mut desktop, "task-header-status", "Todo", |desktop| {
    stored_status(desktop) == WorkflowStatus::Todo
  });
  choose_in_header(&mut desktop, "task-header-status", "Canceled", |desktop| {
    stored_status(desktop) == WorkflowStatus::Canceled
  });
  for _ in 0..5 {
    desktop.pass();
  }
  assert!(desktop.tree.get_element_by_id("task-cancel-confirmation").is_none());
  assert_eq!(stored_status(&desktop), WorkflowStatus::Canceled);
}

/// KON-1, a complete draft of the seeded Delivery, In progress and High.
fn organized_draft(repo: &Path) {
  let mut db = ProjectDb::open(repo).expect("open");
  let pipeline = db.resources(ResourceKind::Pipeline).expect("pipelines")[0].id();
  let id = db
    .create_tasks(IdempotencyKey::generate(), vec![complete(pipeline)], at(10))
    .expect("task")[0]
    .id();
  let organization = TaskOrganization::default()
    .with_status(WorkflowStatus::InProgress)
    .with_priority(Priority::High);
  db.set_task_organization(id, &organization, ChangeActor::Person, at(11))
    .expect("organized");
}

/// R3: Retry run starts the task as stored, so the locked form offers no organization change.
#[test]
fn organization_locked_retry_form_keeps_the_stored_organization() {
  let (runner, fake) = programs();
  let mut desktop = Desktop::with_runner(runner, init_repository, |repo| {
    seed(repo, &fake, false);
    organized_draft(repo);
    unborn_head(repo);
  });
  desktop.wait_for("nav-tasks");
  desktop.wait_until("the runner online", |desktop| {
    crate::task_run::has(desktop, "status-runner") && crate::github::shows(desktop, "status-runner", "Runner online")
  });
  desktop.click("nav-tasks");
  desktop.wait_for("task-row-KON-1");
  desktop.click("task-row-KON-1");
  // The form queues the draft; the runner refuses its run (no commit to start from), so it stays
  // queued without one.
  desktop.wait_for("task-edit");
  desktop.click("task-edit");
  desktop.wait_for("create-and-run");
  desktop.click("create-and-run");
  desktop.wait_until("the queued task", |desktop| {
    crate::github::shows(desktop, "create-and-run", "Retry run")
  });
  // Opened again in this session, the task this form queued opens in the locked form (02.09F).
  desktop.click("nav-tasks");
  desktop.wait_for("task-row-KON-1");
  desktop.click("task-row-KON-1");
  desktop.wait_for("task-workflow-field");
  let stops = desktop.stops(STOPS, false);
  assert!(stops.contains(&Some("create-and-run".into())), "Retry run is available");
  for control in [
    "task-workflow-field",
    "task-priority-field",
    "task-tags-picker",
    "task-blocked-field",
  ] {
    assert!(!stops.contains(&Some(control.into())), "{control} is locked");
  }
  desktop.press("task-blocked-field");
  desktop.press("task-tags-picker");
  desktop.pass();
  assert!(desktop.tree.get_element_by_id("task-blocked-reason").is_none());
  assert!(desktop.tree.get_element_by_id("tag-search").is_none());
  let organization = tasks(&desktop.repo)[0].organization().clone();
  assert_eq!(organization.status(), WorkflowStatus::InProgress);
  assert_eq!(organization.priority(), Priority::High);
}

/// R4: while the header saves a choice its tag picker is unavailable, inline creation included.
#[test]
fn organization_header_tag_picker_creates_no_tag_while_saving() {
  let mut desktop = Desktop::with_seeded_project(|repo| {
    seed_active(repo);
    let mut db = ProjectDb::open(repo).expect("open");
    db.create_project_tag(Name::new("Desktop").expect("name")).expect("tag");
  });
  open_task(&mut desktop);
  let tag = ProjectDb::open(&desktop.repo)
    .expect("open")
    .project_tags()
    .expect("tags")[0]
    .id();
  desktop.press("task-tags-picker");
  desktop.wait_for("tag-search");
  // The header's save waits for the write lock: it stays under way until the lock is released,
  // and a creation pressed meanwhile would complete after it.
  let connection = rusqlite::Connection::open(desktop.repo.join(".orchester/orchester.db")).expect("connection");
  connection
    .execute_batch("BEGIN IMMEDIATE")
    .expect("hold the write lock");
  desktop.press(&format!("picker-tag-{tag}"));
  set_text(&mut desktop, "tag-search", "Client");
  desktop.wait_for("tag-create-inline");
  desktop.press("tag-create-inline");
  connection.execute_batch("ROLLBACK").expect("release the write lock");
  desktop.wait_until("the tag saved", |desktop| {
    tasks(&desktop.repo)[0].organization().tags() == [tag]
  });
  assert_eq!(
    ProjectDb::open(&desktop.repo)
      .expect("open")
      .project_tags()
      .expect("tags")
      .len(),
    1,
    "no tag was created"
  );
}
