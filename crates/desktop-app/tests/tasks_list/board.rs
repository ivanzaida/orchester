//! The Tasks board (ORC-105) on the QA seed's Kontur: the List/Board switch, kept for the project; what a card dropped
//! on a column does by the person's rules (an incomplete draft is refused by Todo, a queued task that never ran returns
//! to draft in Backlog, a task with a run stays out of Backlog, a run that has not ended moves only through the stop
//! confirmation, which says why it failed, a run that ended is offered Run again, Done asks first for work that waits
//! to be merged, In progress changes the status only, columns by pipeline refuse every drop); a canceled card's line;
//! quick add in Backlog; and bulk moves and closes that leave the cards that cannot go with why and ask as single moves
//! do. No runner serves the project, so no agent slot is free and queuing never asks first here; `mcp_domain`'s `board`
//! queues a complete draft and asks with the real runner. `cargo test -p orchester-app --test tasks_list tasks_board_`.

use orchester_app::routes::{ImportTab, ProjectPage, Route, TaskList, TaskListTab};
use orchester_core::{
  store::TaskStore,
  task::{Task, TaskStatus, organization::WorkflowStatus},
};
use orchester_store::ProjectDb;

use crate::harness::Seeded;

/// The seed's task `key` as stored now.
fn stored(seeded: &Seeded, key: &str) -> Task {
  let db = ProjectDb::open(&seeded.repo).expect("the project");
  db.tasks()
    .expect("tasks")
    .into_iter()
    .find(|task| task.key().to_string() == key)
    .expect("the task")
}

fn status(seeded: &Seeded, key: &str) -> WorkflowStatus {
  stored(seeded, key).organization().status()
}

/// Whether the board shows the card of `key` in the column `column`.
fn in_column(seeded: &mut Seeded, key: &str, column: &str) -> bool {
  seeded
    .window
    .texts(&format!("tasks-board-column-{column}"))
    .is_some_and(|texts| texts.iter().any(|text| text == key))
}

/// The board's columns, left to right: a column's own id has no dash after the prefix, which its parts' ids have.
fn columns(seeded: &mut Seeded) -> Vec<String> {
  seeded
    .ids("tasks-board-column-")
    .into_iter()
    .filter(|id| !id.trim_start_matches("tasks-board-column-").contains('-'))
    .collect()
}

/// Presses `id` once it is laid out.
fn press(seeded: &mut Seeded, id: &str) {
  seeded.until(id, |seeded| seeded.laid_out(id));
  seeded.click(id);
}

/// The words of the refusal at the column's top.
fn refusal(seeded: &mut Seeded) -> String {
  seeded.until("the refusal", |seeded| seeded.laid_out("tasks-board-refusal-text"));
  seeded.text("tasks-board-refusal-text")
}

#[test]
fn tasks_board_the_layout_switch_shows_the_columns_and_is_kept_for_the_project() {
  let mut seeded = Seeded::tasks("tasks", TaskList::new(TaskListTab::All));
  assert!(seeded.shows("tasks-list"));
  seeded.click("tasks-layout-board");
  seeded.until("the board", |seeded| seeded.laid_out("tasks-board-columns"));
  assert!(!seeded.shows("tasks-list"));
  assert_eq!(
    columns(&mut seeded),
    [
      "tasks-board-column-backlog",
      "tasks-board-column-todo",
      "tasks-board-column-in_progress",
      "tasks-board-column-in_review",
      "tasks-board-column-done",
    ],
    "the statuses in order, Canceled hidden"
  );
  assert!(in_column(&mut seeded, "KON-3", "backlog"));
  assert!(in_column(&mut seeded, "KON-12", "in_progress"));

  // Away and back: the screen opens on the board again, read from the application database.
  seeded
    .window
    .navigation()
    .go(Route::Project(seeded.project, ProjectPage::Import(ImportTab::All)));
  seeded.window.pass();
  seeded.go(TaskList::new(TaskListTab::All));
  seeded.until("the board again", |seeded| seeded.laid_out("tasks-board-columns"));

  seeded.click("tasks-layout-list");
  seeded.until("the list", |seeded| seeded.shows("tasks-list"));
  assert!(!seeded.shows("tasks-board"));
}

#[test]
fn tasks_board_an_incomplete_draft_is_refused_by_todo() {
  let mut seeded = Seeded::board("tasks", TaskList::new(TaskListTab::All));
  seeded.drag("KON-3", "todo");
  assert_eq!(refusal(&mut seeded), "Add the missing details before running the task.");
  assert_eq!(seeded.text("tasks-board-refusal-title"), "KON-3 can't be queued yet");
  assert_eq!(status(&seeded, "KON-3"), WorkflowStatus::Backlog);
  assert_eq!(stored(&seeded, "KON-3").status(), TaskStatus::Draft);
}

#[test]
fn tasks_board_a_drop_that_fails_snaps_back_and_says_why() {
  let mut seeded = Seeded::board("tasks", TaskList::new(TaskListTab::All));
  // KON-5's Feature delivery has a step whose agent's connection the seed lacks: queuing it fails.
  to_backlog(&mut seeded, "KON-5");
  seeded.drag("KON-5", "todo");
  seeded.until("the failure", |seeded| seeded.laid_out("tasks-board-failed"));
  let said = seeded.text("tasks-board-failed-text");
  assert!(said.starts_with("KON-5 was not changed. "), "{said}");
  assert_eq!(stored(&seeded, "KON-5").status(), TaskStatus::Draft);
  assert_eq!(status(&seeded, "KON-5"), WorkflowStatus::Backlog);
  assert!(
    in_column(&mut seeded, "KON-5", "backlog"),
    "the card is back where it was"
  );
  press(&mut seeded, "tasks-board-failed-dismiss");
  assert!(!seeded.shows("tasks-board-failed"));
}

/// Drops the draft `key` on Backlog and waits until it is there.
fn to_backlog(seeded: &mut Seeded, key: &str) {
  seeded.drag(key, "backlog");
  seeded.until("the draft in Backlog", |seeded| {
    status(seeded, key) == WorkflowStatus::Backlog
  });
  assert_eq!(stored(seeded, key).status(), TaskStatus::Draft);
  seeded.until("its card in Backlog", |seeded| in_column(seeded, key, "backlog"));
}

#[test]
fn tasks_board_a_queued_task_that_never_ran_returns_to_draft_in_backlog_and_one_with_a_run_stays_out() {
  let mut seeded = Seeded::board("tasks", TaskList::new(TaskListTab::All));
  assert_eq!(stored(&seeded, "KON-8").status(), TaskStatus::Queued);
  seeded.drag("KON-8", "backlog");
  seeded.until("KON-8 a draft", |seeded| {
    stored(seeded, "KON-8").status() == TaskStatus::Draft
  });
  assert_eq!(status(&seeded, "KON-8"), WorkflowStatus::Backlog);

  seeded.drag("KON-1", "backlog");
  assert_eq!(refusal(&mut seeded), "This task already has a run.");
  assert_eq!(status(&seeded, "KON-1"), WorkflowStatus::Done);
}

#[test]
fn tasks_board_a_run_that_has_not_ended_moves_only_through_the_stop_confirmation() {
  let mut seeded = Seeded::board("tasks", TaskList::new(TaskListTab::All));
  seeded.drag("KON-12", "todo");
  assert_eq!(
    refusal(&mut seeded),
    "A run of this task has not ended. Answer or stop it from the task, or wait for it to end."
  );
  assert_eq!(status(&seeded, "KON-12"), WorkflowStatus::InProgress);

  seeded.drag("KON-12", "done");
  seeded.until("the stop confirmation", |seeded| {
    seeded.laid_out("tasks-close-stop-keep")
  });
  let asked = seeded.text("tasks-close-stop");
  assert!(asked.contains("Stop the run of KON-12?"), "{asked}");
  press(&mut seeded, "tasks-close-stop-keep");
  assert_eq!(status(&seeded, "KON-12"), WorkflowStatus::InProgress, "kept running");
}

#[test]
fn tasks_board_a_run_that_ended_is_offered_run_again_in_place() {
  let mut seeded = Seeded::board("tasks", TaskList::new(TaskListTab::All));
  seeded.drag("KON-1", "todo");
  seeded.until("the question", |seeded| seeded.laid_out("tasks-board-ask-keep"));
  assert!(seeded.text("tasks-board-ask").starts_with("Run KON-1 again?"));
  press(&mut seeded, "tasks-board-ask-keep");
  assert!(!seeded.shows("tasks-board-ask"));
  assert_eq!(status(&seeded, "KON-1"), WorkflowStatus::Done, "nothing changed");
}

#[test]
fn tasks_board_in_progress_changes_the_status_only_and_never_starts_a_run() {
  let mut seeded = Seeded::board("tasks", TaskList::new(TaskListTab::All));
  seeded.drag("KON-15", "in_progress");
  seeded.until("KON-15 In progress", |seeded| {
    status(seeded, "KON-15") == WorkflowStatus::InProgress
  });
  assert_eq!(stored(&seeded, "KON-15").status(), TaskStatus::Draft, "no run started");
}

#[test]
fn tasks_board_columns_by_pipeline_refuse_every_drop() {
  let mut seeded = Seeded::board("tasks", TaskList::new(TaskListTab::All));
  press(&mut seeded, "tasks-display");
  press(&mut seeded, "tasks-board-options-columns");
  press(&mut seeded, "tasks-board-options-columns-pipeline");
  // KON-12's run pins Feature delivery; KON-4 has no pipeline.
  seeded.until("columns by pipeline", |seeded| {
    seeded.laid_out("tasks-board-column-pipeline-none")
  });
  seeded
    .window
    .tree
    .key_down("Escape".to_owned(), "Escape".to_owned(), false, false, false);
  seeded.until("Display closed", |seeded| !seeded.shows("tasks-board-options"));
  assert!(in_column(&mut seeded, "KON-4", "pipeline-none"));
  assert!(!in_column(&mut seeded, "KON-12", "pipeline-none"));
  seeded.drag("KON-12", "pipeline-none");
  assert_eq!(
    refusal(&mut seeded),
    "Columns by pipeline only show where tasks are; a task's pipeline is chosen on its page."
  );
}

#[test]
fn tasks_board_a_card_canceled_from_its_menu_leaves_its_line_and_the_canceled_strip() {
  let mut seeded = Seeded::board("tasks", TaskList::new(TaskListTab::All));
  seeded.hover("task-card-KON-15");
  press(&mut seeded, "task-more-KON-15");
  press(&mut seeded, "task-menu-KON-15-cancel");
  seeded.until("the line", |seeded| seeded.laid_out("tasks-board-canceled"));
  assert!(seeded.text("tasks-board-canceled").starts_with("KON-15 is Canceled"));
  assert_eq!(status(&seeded, "KON-15"), WorkflowStatus::Canceled);
  assert!(
    seeded.shows("tasks-board-column-canceled"),
    "the Canceled strip with its count"
  );
  press(&mut seeded, "tasks-board-canceled-show");
  seeded.until("Canceled shown", |seeded| in_column(seeded, "KON-15", "canceled"));
}

#[test]
fn tasks_board_quick_add_makes_a_draft_in_backlog_only() {
  let mut seeded = Seeded::board("tasks", TaskList::new(TaskListTab::All));
  assert!(seeded.shows("tasks-board-column-backlog-add"));
  assert!(
    !seeded.shows("tasks-board-column-todo-add"),
    "quick add is Backlog's alone"
  );
  press(&mut seeded, "tasks-board-new-task");
  seeded.until("the field", |seeded| seeded.laid_out("tasks-board-quick-add-title"));
  seeded
    .window
    .set("tasks-board-quick-add-title", "Write the board's docs");
  seeded
    .window
    .tree
    .key_down("Enter".to_owned(), "Enter".to_owned(), false, false, false);
  seeded.window.pass();
  seeded.until("the draft", |seeded| in_column(seeded, "KON-25", "backlog"));
  let added = stored(&seeded, "KON-25");
  assert_eq!(added.details().title.as_str(), "Write the board's docs");
  assert_eq!(added.status(), TaskStatus::Draft);
  assert_eq!(added.organization().status(), WorkflowStatus::Backlog);
}

#[test]
fn tasks_board_a_bulk_move_leaves_the_cards_that_cannot_go_and_says_why() {
  let mut seeded = Seeded::board("tasks", TaskList::new(TaskListTab::All));
  seeded.shift_click("task-card-KON-12");
  seeded.shift_click("task-card-KON-15");
  seeded.until("the bulk bar", |seeded| seeded.laid_out("tasks-board-bulk-move"));
  assert!(seeded.text("tasks-board-bulk").contains("2 selected"));
  press(&mut seeded, "tasks-board-bulk-move");
  seeded.until("the menu", |seeded| seeded.laid_out("tasks-board-bulk-move-backlog"));
  let row = seeded.text("tasks-board-bulk-move-backlog");
  assert!(row.contains("KON-12 stay: A run of this task has not ended."), "{row}");
  press(&mut seeded, "tasks-board-bulk-move-backlog");
  seeded.until("KON-15 in Backlog", |seeded| {
    status(seeded, "KON-15") == WorkflowStatus::Backlog
  });
  assert_eq!(status(&seeded, "KON-12"), WorkflowStatus::InProgress, "KON-12 stays");
}

#[test]
fn tasks_board_ready_to_merge_on_done_asks_first() {
  let mut seeded = Seeded::board("tasks", TaskList::new(TaskListTab::All));
  // KON-9's run passed every step with work nobody merged.
  seeded.drag("KON-9", "done");
  seeded.until("the question", |seeded| seeded.laid_out("tasks-board-ask-keep"));
  assert_eq!(
    seeded.text("tasks-board-ask-title"),
    "Mark KON-9 as done without merging?"
  );
  press(&mut seeded, "tasks-board-ask-keep");
  assert!(!seeded.shows("tasks-board-ask"));
  assert_eq!(status(&seeded, "KON-9"), WorkflowStatus::InReview, "kept in review");

  seeded.drag("KON-9", "done");
  press(&mut seeded, "tasks-board-ask-confirm");
  seeded.until("KON-9 Done", |seeded| status(seeded, "KON-9") == WorkflowStatus::Done);
}

#[test]
fn tasks_board_cancel_of_a_run_that_has_not_ended_goes_through_the_stop_confirmation_and_says_why_it_failed() {
  let mut seeded = Seeded::board("tasks", TaskList::new(TaskListTab::All));
  seeded.hover("task-card-KON-12");
  press(&mut seeded, "task-more-KON-12");
  press(&mut seeded, "task-menu-KON-12-cancel");
  seeded.until("the stop confirmation", |seeded| {
    seeded.laid_out("tasks-close-stop-confirm")
  });
  assert_eq!(seeded.text("tasks-close-stop-title"), "Stop the run of KON-12?");
  // No runner serves the project: the run cannot be stopped, the confirmation says why and the task stays.
  press(&mut seeded, "tasks-close-stop-confirm");
  seeded.until("why it failed", |seeded| seeded.laid_out("tasks-close-stop-error"));
  let said = seeded.text("tasks-close-stop-error");
  assert!(said.contains("runner"), "{said}");
  assert_eq!(status(&seeded, "KON-12"), WorkflowStatus::InProgress);
}

#[test]
fn tasks_board_bulk_mark_as_done_asks_for_work_waiting_to_be_merged_and_for_runs_not_ended() {
  let mut seeded = Seeded::board("tasks", TaskList::new(TaskListTab::All));
  seeded.shift_click("task-card-KON-9");
  seeded.shift_click("task-card-KON-15");
  press(&mut seeded, "tasks-board-bulk-done");
  seeded.until("the question", |seeded| seeded.laid_out("tasks-board-ask-keep"));
  assert_eq!(
    seeded.text("tasks-board-ask-title"),
    "Mark 2 tasks as done without merging?"
  );
  assert!(
    seeded
      .text("tasks-board-ask-text")
      .starts_with("Ready to merge: KON-9.")
  );
  press(&mut seeded, "tasks-board-ask-keep");
  assert_eq!(status(&seeded, "KON-9"), WorkflowStatus::InReview, "nothing changed");
  assert_eq!(status(&seeded, "KON-15"), WorkflowStatus::Todo);

  // A run that has not ended is stopped only once the person says so, as a single card's close asks.
  seeded.shift_click("task-card-KON-9");
  seeded.shift_click("task-card-KON-12");
  press(&mut seeded, "tasks-board-bulk-cancel");
  seeded.until("the stop question", |seeded| seeded.laid_out("tasks-board-ask-keep"));
  assert_eq!(seeded.text("tasks-board-ask-title"), "Stop 1 runs and cancel 2 tasks?");
  press(&mut seeded, "tasks-board-ask-keep");
  assert_eq!(status(&seeded, "KON-12"), WorkflowStatus::InProgress);
  assert_eq!(status(&seeded, "KON-15"), WorkflowStatus::Todo);

  // KON-9 and KON-15 again: the person's yes closes both, KON-9 unmerged.
  seeded.shift_click("task-card-KON-12");
  seeded.shift_click("task-card-KON-9");
  press(&mut seeded, "tasks-board-bulk-done");
  press(&mut seeded, "tasks-board-ask-confirm");
  seeded.until("both Done", |seeded| {
    status(seeded, "KON-9") == WorkflowStatus::Done && status(seeded, "KON-15") == WorkflowStatus::Done
  });
}

#[test]
fn tasks_board_a_card_whose_run_has_not_ended_finds_done_and_canceled_offered_wherever_it_is_held() {
  let mut seeded = Seeded::board("tasks", TaskList::new(TaskListTab::All));
  // Held over In review, which refuses it: Done already offers Mark as done and Cancel task (`03 Drop refused`).
  let ((x0, y0), _) = seeded.hold("KON-12", "in_review");
  seeded.until("Done's slots", |seeded| seeded.laid_out("tasks-board-slot-done"));
  assert!(seeded.laid_out("tasks-board-slot-canceled"));
  assert_eq!(
    seeded.text("tasks-board-hint-text"),
    "KON-12 goes only to Done or Canceled"
  );
  // Let go where it was taken from: nothing changes.
  seeded.window.tree.mouse_move(x0, y0);
  seeded.window.pass();
  seeded
    .window
    .tree
    .mouse_up(x0, y0, lurq::app::events::MouseButton::Left);
  seeded.window.pass();
  assert!(!seeded.shows("tasks-board-slot-done"), "the slots go with the drag");
  assert_eq!(status(&seeded, "KON-12"), WorkflowStatus::InProgress);
}

#[test]
fn tasks_board_move_to_opens_beside_the_menu_and_says_once_why_columns_are_faded() {
  let mut seeded = Seeded::board("tasks", TaskList::new(TaskListTab::All));
  seeded.hover("task-card-KON-12");
  press(&mut seeded, "task-more-KON-12");
  press(&mut seeded, "task-menu-KON-12-move");
  seeded.until("the list", |seeded| seeded.laid_out("task-menu-KON-12-list"));
  assert!(
    seeded.laid_out("task-menu-KON-12-copy"),
    "the menu stays beside its list"
  );
  assert_eq!(
    seeded.text("task-menu-KON-12-move-backlog"),
    "Backlog",
    "no reason on the row"
  );
  assert_eq!(
    seeded.text("task-menu-KON-12-move-reason"),
    "A run of this task has not ended. Done and Canceled stop it, after you confirm."
  );
  press(&mut seeded, "task-menu-KON-12-move-done");
  seeded.until("the stop confirmation", |seeded| {
    seeded.laid_out("tasks-close-stop-keep")
  });
  assert!(
    !seeded.shows("task-menu-KON-12"),
    "a choice closes the menu and its list"
  );
  press(&mut seeded, "tasks-close-stop-keep");
  assert_eq!(status(&seeded, "KON-12"), WorkflowStatus::InProgress);

  // A task with a run: Backlog alone is faded, named in the reason.
  seeded.hover("task-card-KON-1");
  press(&mut seeded, "task-more-KON-1");
  press(&mut seeded, "task-menu-KON-1-move");
  seeded.until("the list", |seeded| seeded.laid_out("task-menu-KON-1-move-reason"));
  assert_eq!(
    seeded.text("task-menu-KON-1-move-reason"),
    "Backlog: This task already has a run."
  );
}
