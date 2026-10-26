//! Closing and deleting a task by hand (ORC-91) from a row's *More actions* menu and from the task page's header:
//! KON-11, whose run failed, is canceled and reopened, and deleted after its confirmation in place says what goes
//! with it, by a person only; KON-12, whose run has not ended, asks first whether to stop that run, and is not
//! deleted. `cargo test -p orchester-app --test tasks_list tasks_close_`.

use lurq::app::events::with_synthetic_input;
use orchester_app::routes::{ProjectPage, Route, TaskList, TaskListTab, TaskView};
use orchester_core::{
  store::TaskStore,
  task::{Task, organization::WorkflowStatus},
};
use orchester_store::ProjectDb;

use crate::harness::Seeded;

/// The seed's task `key` as stored now; `None` once it was deleted.
fn stored(seeded: &Seeded, key: &str) -> Option<Task> {
  let db = ProjectDb::open(&seeded.repo).expect("the project");
  db.tasks()
    .expect("tasks")
    .into_iter()
    .find(|task| task.key().to_string() == key)
}

fn status(seeded: &Seeded, key: &str) -> WorkflowStatus {
  stored(seeded, key).expect("the task").organization().status()
}

/// Opens the *More actions* menu `menu` and chooses its row `choice`, each once laid out. A row's menu shows under the
/// pointer: it moves onto the row first.
fn choose(seeded: &mut Seeded, menu: &str, choice: &str) {
  if let Some(key) = menu.strip_prefix("task-more-") {
    let row = format!("task-row-{key}");
    seeded.until(&row, |seeded| seeded.laid_out(&row));
    seeded.hover(&row);
  }
  press(seeded, menu);
  press(seeded, &format!("{menu}-{choice}"));
}

/// Presses `id` once it is laid out.
fn press(seeded: &mut Seeded, id: &str) {
  seeded.until(id, |seeded| seeded.laid_out(id));
  seeded.click(id);
}

/// The group KON-11's row is under in All, as the list reads again after a change.
fn group_of(seeded: &mut Seeded, key: &str) -> Option<String> {
  seeded
    .groups()
    .into_iter()
    .find(|(_, keys)| keys.iter().any(|shown| shown == key))
    .map(|(group, _)| group)
}

#[test]
fn tasks_close_a_task_whose_run_failed_from_its_row_and_reopen_it() {
  let mut seeded = Seeded::tasks("tasks", TaskList::new(TaskListTab::All));
  assert_eq!(group_of(&mut seeded, "KON-11").as_deref(), Some("in_progress"));

  choose(&mut seeded, "task-more-KON-11", "cancel");
  seeded.until("KON-11 Canceled", |seeded| {
    group_of(seeded, "KON-11").as_deref() == Some("canceled")
  });
  assert_eq!(status(&seeded, "KON-11"), WorkflowStatus::Canceled);

  choose(&mut seeded, "task-more-KON-11", "reopen");
  seeded.until("KON-11 reopened", |seeded| {
    group_of(seeded, "KON-11").as_deref() == Some("todo")
  });
  assert_eq!(
    status(&seeded, "KON-11"),
    WorkflowStatus::Todo,
    "a submitted task reopens to Todo"
  );

  choose(&mut seeded, "task-more-KON-11", "done");
  seeded.until("KON-11 Done", |seeded| {
    group_of(seeded, "KON-11").as_deref() == Some("done")
  });
}

#[test]
fn tasks_close_a_task_whose_run_has_not_ended_asks_in_place_first() {
  let mut seeded = Seeded::tasks("tasks", TaskList::default());
  choose(&mut seeded, "task-more-KON-12", "done");
  seeded.until("the stop confirmation", |seeded| {
    seeded.laid_out("tasks-close-stop-keep")
  });
  let asked = seeded.text("tasks-close-stop");
  assert!(asked.contains("Stop the run of KON-12?"), "{asked}");
  assert!(asked.contains("marks KON-12 as Done"), "{asked}");
  assert!(asked.contains("Stop run and mark as done"), "{asked}");
  assert_eq!(
    status(&seeded, "KON-12"),
    WorkflowStatus::InProgress,
    "nothing changed yet"
  );

  press(&mut seeded, "tasks-close-stop-keep");
  assert!(
    !seeded.shows("tasks-close-stop"),
    "Keep it running closes the confirmation"
  );
  assert_eq!(status(&seeded, "KON-12"), WorkflowStatus::InProgress);
}

#[test]
fn tasks_close_delete_says_what_goes_and_removes_the_task() {
  let mut seeded = Seeded::tasks("tasks", TaskList::default());
  choose(&mut seeded, "task-more-KON-11", "delete");
  seeded.until("what goes with KON-11", |seeded| {
    seeded.laid_out("tasks-close-delete-text")
  });
  assert_eq!(seeded.text("tasks-close-delete-title"), "Delete KON-11?");
  let said = seeded.text("tasks-close-delete-text");
  assert!(
    said.starts_with("KON-11 goes with its run and the run's history"),
    "{said}"
  );
  assert!(said.ends_with("This can't be undone."), "{said}");
  assert!(
    stored(&seeded, "KON-11").is_some(),
    "nothing is deleted before the press"
  );

  press(&mut seeded, "tasks-close-delete-confirm");
  seeded.until("KON-11 deleted", |seeded| !seeded.shows("task-row-KON-11"));
  assert!(stored(&seeded, "KON-11").is_none(), "the task is gone");
  assert!(!seeded.shows("tasks-close-delete"), "the confirmation closed");
}

#[test]
fn tasks_close_delete_by_an_mcp_client_deletes_nothing_and_says_only_a_person_deletes() {
  let mut seeded = Seeded::tasks("tasks", TaskList::default());
  choose(&mut seeded, "task-more-KON-11", "delete");
  seeded.until("the delete confirmation", |seeded| {
    seeded.laid_out("tasks-close-delete-confirm")
  });
  seeded.until("what goes with KON-11", |seeded| {
    seeded.laid_out("tasks-close-delete-text")
  });

  with_synthetic_input(|| seeded.click("tasks-close-delete-confirm"));
  seeded.until("why it was refused", |seeded| {
    seeded.laid_out("tasks-close-delete-person-only")
  });
  assert_eq!(
    seeded.text("tasks-close-delete-person-only"),
    "Only a person deletes a task: input from an MCP client is not taken here."
  );
  assert!(stored(&seeded, "KON-11").is_some(), "nothing was deleted");
}

#[test]
fn tasks_close_delete_of_a_task_whose_run_has_not_ended_is_unavailable() {
  let mut seeded = Seeded::tasks("tasks", TaskList::default());
  choose(&mut seeded, "task-more-KON-12", "delete");
  seeded.until("why KON-12 is not deleted", |seeded| {
    seeded.laid_out("tasks-close-delete-unavailable")
  });
  assert_eq!(
    seeded.text("tasks-close-delete-unavailable"),
    "A run of this task has not ended. Stop it before deleting the task."
  );
  press(&mut seeded, "tasks-close-delete-confirm");
  assert!(stored(&seeded, "KON-12").is_some(), "Delete task is unavailable");
}

#[test]
fn tasks_close_from_the_task_page_header_then_delete_returns_to_the_list() {
  let mut seeded = Seeded::tasks("tasks", TaskList::default());
  let key = "KON-11".parse().expect("a task key");
  let page = Route::Project(seeded.project, ProjectPage::Task(key, TaskView::default()));
  seeded.window.navigation().go(page);

  choose(&mut seeded, "task-more", "done");
  seeded.until("KON-11 Done", |seeded| status(seeded, "KON-11") == WorkflowStatus::Done);

  choose(&mut seeded, "task-more", "delete");
  seeded.until("the delete confirmation", |seeded| {
    seeded.laid_out("task-close-delete-text")
  });
  press(&mut seeded, "task-close-delete-confirm");
  seeded.until("the Tasks list", |seeded| seeded.shows("screen-tasks"));
  assert!(stored(&seeded, "KON-11").is_none(), "the task is gone");
}
