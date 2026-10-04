//! Edit task when the task was saved meanwhile (10.41 `RmDpQ`, decisions 17 and 18): a change of
//! another field merges without asking; a field both changed shows both versions, nothing of the
//! person's is saved until they choose, and the choice is saved on top of the newer version. For a
//! shared task the view names who saved it, and the choice reaches the server.

use std::path::Path;

use orchester_api::{TaskChanges, TaskPatch};
use orchester_core::{
  id::IdempotencyKey,
  name::Name,
  store::TaskStore,
  task::{Task, TaskDetails, TaskUpdate},
  time::Timestamp,
};
use orchester_store::ProjectDb;

use crate::{
  agent_permissions::shows,
  harness::Desktop,
  sharing::{add_member, elsewhere, share, signed_in},
  task_run::set_text,
};

const ROW: &str = "task-row-KON-1";

fn draft() -> TaskDetails {
  TaskDetails {
    title: Name::new("Token usage export to CSV").expect("title"),
    brief: "Export token usage.".into(),
    acceptance_criteria: vec!["The export covers the selected period".into()],
    pipeline: None,
    limits: None,
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
  assert_eq!(desktop.value("task-brief"), "Export one row per run step.");
  assert_eq!(desktop.value("criterion-1"), "Totals match the dashboard");
}

#[test]
fn edit_conflict_a_shared_task_names_who_saved_it_and_takes_their_version() {
  let (mut desktop, server) = signed_in("alice", true, &["Token usage export to CSV"]);
  let (client, bohdan) = elsewhere(&server, "bohdan", "Bohdan Kravets");
  share(&mut desktop);
  add_member(&mut desktop, "bohdan");
  desktop.wait_until("Bohdan's row", |desktop| shows(desktop, "Bohdan Kravets"));
  open_edit(&mut desktop);
  set_text(&mut desktop, "task-brief", "Alice's outcome.");
  let project = ProjectDb::open(desktop.repo())
    .expect("project")
    .sharing()
    .expect("link")
    .expect("shared")
    .project;
  let task = server
    .block_on(client.tasks(&bohdan, project))
    .expect("Bohdan reads the tasks")
    .tasks
    .into_iter()
    .next()
    .expect("the task");
  let patch = TaskPatch {
    revision: task.revision,
    changes: TaskChanges {
      brief: Some("Bohdan's outcome.".into()),
      ..TaskChanges::default()
    },
  };
  server
    .block_on(client.update_task(&bohdan, project, task.id, &patch))
    .expect("Bohdan saves first");
  desktop.click("save-draft");
  desktop.wait_for("task-conflict");
  state_reads(&mut desktop, "Not saved · Bohdan Kravets saved a newer version");
  assert!(shows(&mut desktop, "Bohdan Kravets saved KON-1"));
  assert!(shows(&mut desktop, "Bohdan Kravets's version"));
  desktop.click("task-conflict-use-theirs");
  desktop.wait_until("Bohdan's version stays on the server", |desktop| {
    desktop.tree.get_element_by_id("task-brief").is_some() && desktop.value("task-brief") == "Bohdan's outcome."
  });
  let stored = server
    .block_on(client.tasks(&bohdan, project))
    .expect("tasks")
    .tasks
    .into_iter()
    .next()
    .expect("the task");
  assert_eq!(stored.content.brief, "Bohdan's outcome.");
}
