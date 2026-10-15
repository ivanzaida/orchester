//! Edit task when the task was saved meanwhile (10.41 `RmDpQ`, decisions 17 and 18): a change of
//! another field merges without asking; a field both changed shows both versions, nothing of the
//! person's is saved until they choose, and the choice is saved on top of the newer version..

use std::path::Path;

use orchester_core::{
  id::IdempotencyKey,
  name::Name,
  store::TaskStore,
  task::{Task, TaskDetails, TaskLimits, TaskUpdate},
  time::Timestamp,
};
use orchester_store::ProjectDb;

use crate::{agent_permissions::shows, harness::Desktop, task_run::set_text};

const ROW: &str = "task-row-KON-1";

fn draft() -> TaskDetails {
  TaskDetails {
    title: Name::new("Token usage export to CSV").expect("title"),
    brief: "Export token usage.".into(),
    acceptance_criteria: vec!["The export covers the selected period".into()],
    pipeline: None,
    limits: TaskLimits::NONE,
  }
}

fn stored(repo: &Path) -> Task {
  ProjectDb::open(repo)
    .expect("open the project")
    .tasks()
    .expect("tasks")
    .into_iter()
    .next()
    .expect("the task")
}

/// Changes the stored task as another window of this computer would.
fn change_meanwhile(repo: &Path, change: impl FnOnce(&mut TaskDetails)) {
  let task = stored(repo);
  let mut details = task.details().clone();
  change(&mut details);
  let mut db = ProjectDb::open(repo).expect("open the project");
  let update = db
    .update_task(task.id(), task.revision(), details)
    .expect("the change is saved");
  assert!(matches!(update, TaskUpdate::Saved(_)), "the change meanwhile is saved");
}

/// Opens the first task's Edit task form.
fn open_edit(desktop: &mut Desktop) {
  desktop.wait_for("nav-tasks");
  desktop.click("nav-tasks");
  desktop.wait_for(ROW);
  desktop.press(ROW);
  desktop.wait_for("task-edit");
  desktop.click("task-edit");
  desktop.wait_for("task-title");
}

fn state_reads(desktop: &mut Desktop, text: &str) {
  desktop.settle(text, |tree| {
    tree
      .get_element_by_id("new-task-state")
      .and_then(|state| state.text_content())
      .is_some_and(|state| state.contains(text))
  });
}

#[test]
fn edit_conflict_a_change_of_another_field_merges_without_asking() {
  let mut desktop = Desktop::with_seeded_project(|repo| {
    ProjectDb::open(repo)
      .expect("open the project")
      .create_tasks(IdempotencyKey::generate(), vec![draft()], Timestamp::now())
      .expect("the draft is saved");
  });
  open_edit(&mut desktop);
  set_text(&mut desktop, "task-title", "Usage export to CSV");
  change_meanwhile(desktop.repo(), |details| {
    details.acceptance_criteria.push("Totals match the dashboard".into());
  });
  desktop.click("save-draft");
  desktop.wait_until("the merged change is saved", |desktop| {
    let task = stored(desktop.repo());
    task.details().title.as_str() == "Usage export to CSV" && task.details().acceptance_criteria.len() == 2
  });
  assert!(desktop.tree.get_element_by_id("task-conflict").is_none());
  desktop.wait_until("the form shows the merged criteria", |desktop| {
    desktop.tree.get_element_by_id("criterion-1").is_some()
      && desktop.value("criterion-1") == "Totals match the dashboard"
  });
}

#[test]
fn edit_conflict_both_changed_the_outcome_and_the_person_chooses() {
  let mut desktop = Desktop::with_seeded_project(|repo| {
    ProjectDb::open(repo)
      .expect("open the project")
      .create_tasks(IdempotencyKey::generate(), vec![draft()], Timestamp::now())
      .expect("the draft is saved");
  });
  open_edit(&mut desktop);
  set_text(&mut desktop, "task-brief", "Export one row per run step.");
  change_meanwhile(desktop.repo(), |details| {
    details.brief = "Export one row per run.".into();
    details.acceptance_criteria.push("Totals match the dashboard".into());
  });
  desktop.click("save-draft");
  desktop.wait_for("task-conflict");
  state_reads(&mut desktop, "Not saved · a newer version was saved");
  assert!(shows(&mut desktop, "KON-1 was saved elsewhere while you were editing"));
  assert!(shows(&mut desktop, "You both changed the outcome"));
  assert!(shows(&mut desktop, "Changed by you and elsewhere"));
  assert!(shows(&mut desktop, "Changed elsewhere · kept"));
  assert!(shows(&mut desktop, "added meanwhile"));
  desktop.wait_for("task-conflict-brief-mine");
  assert_eq!(
    stored(desktop.repo()).details().brief,
    "Export one row per run.",
    "nothing of the person's is saved before they choose"
  );
  desktop.click("task-conflict-brief-theirs");
  desktop.click("task-conflict-brief-mine");
  desktop.click("task-conflict-save");
  desktop.wait_until("the person's outcome is saved on top", |desktop| {
    let task = stored(desktop.repo());
    task.details().brief == "Export one row per run step." && task.details().acceptance_criteria.len() == 2
  });
  desktop.wait_for("task-brief");
  state_reads(&mut desktop, "saved");
  let activity = desktop.activity.snapshot();
  assert!(
    !matches!(activity.slot(), orchester_desktop::activity::Slot::Failure(_)),
    "a newer version is not a failure in the status bar"
  );
  assert_eq!(desktop.value("task-brief"), "Export one row per run step.");
  assert_eq!(desktop.value("criterion-1"), "Totals match the dashboard");
}
