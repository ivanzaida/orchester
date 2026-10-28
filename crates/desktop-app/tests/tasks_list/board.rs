//! The Tasks board (ORC-105) on the QA seed's Kontur: the List/Board switch, kept for the project; what a card dropped
//! on a column does by the person's rules (an incomplete draft is refused by Todo, a queued task that never ran returns
//! to draft in Backlog, a task with a run stays out of Backlog, a run that has not ended moves only through the stop
//! confirmation, which says why it failed, a run that failed dropped on Todo is asked Don't move, Move to Todo or Run
//! again, alone and in bulk, while one that succeeded only moves (ORC-144), Done asks first for work that waits to be
//! merged, In progress changes the status
//! only, columns by pipeline refuse every drop); a canceled card's line; quick add in Backlog; and bulk moves and
//! closes that leave the cards that cannot go with why and ask as single moves do. Nothing a drag or a question shows
//! moves a card (`board_layout`). No runner serves the project, so no agent slot is free and queuing never asks first
//! here, and Run again is refused by the missing runner; `mcp_domain`'s `board` queues a complete draft and asks with
//! the real runner. `cargo test -p orchester-app --test tasks_list tasks_board_`.

use orchester_app::routes::{ImportTab, ProjectPage, Route, TaskList, TaskListTab};
use orchester_core::{
  store::{RunStore, TaskStore},
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
pub fn in_column(seeded: &mut Seeded, key: &str, column: &str) -> bool {
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
pub fn press(seeded: &mut Seeded, id: &str) {
  seeded.until(id, |seeded| seeded.laid_out(id));
  seeded.click(id);
}

/// The few words of the refusal chip over the column `column`.
pub fn refusal(seeded: &mut Seeded, column: &str) -> String {
  let id = format!("tasks-board-column-{column}-refused-text");
  seeded.until("the refusal", |seeded| seeded.laid_out(&id));
  seeded.text(&id)
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
  assert_eq!(refusal(&mut seeded, "todo"), "Needs a brief: add it before queuing");
  assert_eq!(status(&seeded, "KON-3"), WorkflowStatus::Backlog);
  assert_eq!(stored(&seeded, "KON-3").status(), TaskStatus::Draft);
  // The chip stays over the column, its cards where they were, until Esc.
  seeded
    .window
    .tree
    .key_down("Escape".to_owned(), "Escape".to_owned(), false, false, false);
  seeded.window.pass();
  assert!(!seeded.shows("tasks-board-column-todo-refused"));
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
  assert_eq!(refusal(&mut seeded, "backlog"), "Has a run: can't go back to Backlog");
  assert_eq!(status(&seeded, "KON-1"), WorkflowStatus::Done);
}

#[test]
fn tasks_board_a_run_that_has_not_ended_moves_only_through_the_stop_confirmation() {
  let mut seeded = Seeded::board("tasks", TaskList::new(TaskListTab::All));
  seeded.drag("KON-12", "todo");
  assert_eq!(refusal(&mut seeded, "todo"), "Run not ended: stop it from the task");
  assert_eq!(status(&seeded, "KON-12"), WorkflowStatus::InProgress);

  seeded.drag("KON-12", "done");
  seeded.until("the stop confirmation", |seeded| {
    seeded.laid_out("tasks-close-stop-keep")
  });
  let asked = seeded.text("tasks-close-stop");
  assert!(asked.contains("Stop the run of KON-12?"), "{asked}");
  press(&mut seeded, "tasks-close-stop-keep");
  assert!(!seeded.shows("tasks-close-stop"));
  assert!(!seeded.shows("task-card-KON-12-pending"), "the card back in its place");
  assert_eq!(status(&seeded, "KON-12"), WorkflowStatus::InProgress, "kept running");
}

/// How many runs the seed's task `key` has.
fn runs(seeded: &Seeded, key: &str) -> usize {
  let task = stored(seeded, key);
  ProjectDb::open(&seeded.repo)
    .expect("the project")
    .runs_of_task(task.id())
    .expect("runs")
    .len()
}

fn key(seeded: &mut Seeded, name: &str) {
  seeded
    .window
    .tree
    .key_down(name.to_owned(), name.to_owned(), false, false, false);
  seeded.window.pass();
}

/// Drops `key`'s card on Todo and waits for the question of a task whose run ended, with its three answers.
fn asked_to_run_again(seeded: &mut Seeded, card: &str) {
  seeded.drag(card, "todo");
  seeded.until("the question", |seeded| seeded.laid_out("tasks-board-ask-move"));
  assert_eq!(seeded.text("tasks-board-ask-title"), format!("Run {card} again?"));
  assert_eq!(seeded.text("tasks-board-ask-keep"), "Don't move");
  assert_eq!(seeded.text("tasks-board-ask-move"), "Move to Todo");
  assert_eq!(seeded.text("tasks-board-ask-confirm"), "Run again");
}

#[test]
fn tasks_board_a_run_that_ended_dropped_on_todo_offers_don_t_move_move_to_todo_and_run_again() {
  let mut seeded = Seeded::board("tasks", TaskList::new(TaskListTab::All));
  // KON-11's run failed: Run again may follow it.
  let ran = runs(&seeded, "KON-11");
  asked_to_run_again(&mut seeded, "KON-11");
  let said = seeded.text("tasks-board-ask");
  assert!(said.contains("Move to Todo starts nothing."), "{said}");
  assert!(
    said.contains("runs again") && said.contains("doesn't move it"),
    "{said}"
  );
  // What each answer does fits in two lines of the question's words, the title being one.
  let (title, detail) = (
    seeded.bounds("tasks-board-ask-title"),
    seeded.bounds("tasks-board-ask-text"),
  );
  assert!(
    detail.height < title.height * 2.5,
    "{detail:?} against one line {title:?}"
  );
  // The three buttons in a row at the question's end, Run again last.
  let (keep, moving, confirm) = (
    seeded.bounds("tasks-board-ask-keep"),
    seeded.bounds("tasks-board-ask-move"),
    seeded.bounds("tasks-board-ask-confirm"),
  );
  assert!(keep.x < moving.x && moving.x < confirm.x);
  assert!((keep.y - confirm.y).abs() < 0.5 && (moving.y - confirm.y).abs() < 0.5);
  let panel = seeded.bounds("tasks-board-ask");
  assert!(confirm.x + confirm.width <= panel.x + panel.width, "inside the popover");

  // Don't move, then Esc: the card stays where it was.
  press(&mut seeded, "tasks-board-ask-keep");
  assert!(!seeded.shows("tasks-board-ask"));
  assert_eq!(status(&seeded, "KON-11"), WorkflowStatus::InProgress, "nothing changed");
  asked_to_run_again(&mut seeded, "KON-11");
  key(&mut seeded, "Escape");
  assert!(!seeded.shows("tasks-board-ask"));
  assert_eq!(
    status(&seeded, "KON-11"),
    WorkflowStatus::InProgress,
    "Esc moves nothing"
  );
  assert!(in_column(&mut seeded, "KON-11", "in_progress"));

  // Move to Todo: the status only, no run.
  asked_to_run_again(&mut seeded, "KON-11");
  press(&mut seeded, "tasks-board-ask-move");
  seeded.until("KON-11 in Todo", |seeded| {
    status(seeded, "KON-11") == WorkflowStatus::Todo
  });
  seeded.until("its card in Todo", |seeded| in_column(seeded, "KON-11", "todo"));
  assert_eq!(runs(&seeded, "KON-11"), ran, "no run started");
  assert!(!seeded.shows("tasks-board-failed"));

  // From Todo, the card's Run again asks again without a move, and Enter runs it again: no runner serves the project,
  // so the run is refused and says why.
  seeded.hover("task-card-KON-11");
  press(&mut seeded, "task-more-KON-11");
  press(&mut seeded, "task-menu-KON-11-run-again");
  seeded.until("the question", |seeded| seeded.laid_out("tasks-board-ask-confirm"));
  assert!(!seeded.shows("tasks-board-ask-move"), "in Todo already");
  assert_eq!(seeded.text("tasks-board-ask-keep"), "Keep in Todo");
  key(&mut seeded, "Enter");
  seeded.until("the run refused", |seeded| seeded.laid_out("tasks-board-failed"));
  let said = seeded.text("tasks-board-failed-text");
  assert!(said.starts_with("KON-11 was not changed. "), "{said}");
  assert_eq!(status(&seeded, "KON-11"), WorkflowStatus::Todo);
}

#[test]
fn tasks_board_run_again_from_the_question_asks_the_runner_for_a_new_run() {
  let mut seeded = Seeded::board("tasks", TaskList::new(TaskListTab::All));
  asked_to_run_again(&mut seeded, "KON-11");
  // No runner serves the project: Run again asks it for the new run, which is refused and says so.
  press(&mut seeded, "tasks-board-ask-confirm");
  seeded.until("the run refused", |seeded| seeded.laid_out("tasks-board-failed"));
  let said = seeded.text("tasks-board-failed-text");
  assert!(said.starts_with("KON-11 was not changed. "), "{said}");
  assert!(said.contains("runner"), "{said}");
  assert_eq!(
    status(&seeded, "KON-11"),
    WorkflowStatus::InProgress,
    "not moved without its run"
  );
}

#[test]
fn tasks_board_a_run_that_succeeded_never_runs_again_so_todo_only_moves_it() {
  let mut seeded = Seeded::board("tasks", TaskList::new(TaskListTab::All));
  // KON-9's run passed every step with work nobody merged; KON-1's was merged. Core's run_again refuses both.
  let ran = (runs(&seeded, "KON-9"), runs(&seeded, "KON-1"));
  seeded.hover("task-card-KON-9");
  press(&mut seeded, "task-more-KON-9");
  seeded.until("the card's menu", |seeded| seeded.laid_out("task-menu-KON-9-move"));
  assert!(!seeded.shows("task-menu-KON-9-run-again"), "no Run again to offer");
  key(&mut seeded, "Escape");
  seeded.until("the menu closed", |seeded| !seeded.shows("task-menu-KON-9-move"));

  for (card, column) in [("KON-9", "in_review"), ("KON-1", "done")] {
    assert!(in_column(&mut seeded, card, column));
    seeded.drag(card, "todo");
    seeded.until("the move", |seeded| status(seeded, card) == WorkflowStatus::Todo);
    assert!(!seeded.shows("tasks-board-ask"), "nothing asked: {card} only moves");
    seeded.until("its card in Todo", |seeded| in_column(seeded, card, "todo"));
    assert!(!seeded.shows("tasks-board-failed"));
  }
  assert_eq!((runs(&seeded, "KON-9"), runs(&seeded, "KON-1")), ran, "no run started");
}

#[test]
fn tasks_board_a_bulk_move_to_todo_of_tasks_whose_runs_ended_asks_the_same_three_ways() {
  let mut seeded = Seeded::board("tasks", TaskList::new(TaskListTab::All));
  let ran = (runs(&seeded, "KON-1"), runs(&seeded, "KON-11"));
  // KON-11's failed run may be followed by Run again; KON-1's merged run never is, so only KON-11 is asked about.
  seeded.shift_click("task-card-KON-1");
  seeded.shift_click("task-card-KON-11");
  press(&mut seeded, "tasks-board-bulk-move");
  press(&mut seeded, "tasks-board-bulk-move-todo");
  seeded.until("the question", |seeded| seeded.laid_out("tasks-board-ask-move"));
  assert_eq!(seeded.text("tasks-board-ask-title"), "Run KON-11 again?");
  assert_eq!(seeded.text("tasks-board-ask-keep"), "Don't move");
  press(&mut seeded, "tasks-board-ask-keep");
  assert_eq!(status(&seeded, "KON-1"), WorkflowStatus::Done, "nothing moved");
  assert_eq!(status(&seeded, "KON-11"), WorkflowStatus::InProgress);

  press(&mut seeded, "tasks-board-bulk-move");
  press(&mut seeded, "tasks-board-bulk-move-todo");
  press(&mut seeded, "tasks-board-ask-move");
  seeded.until("both in Todo", |seeded| {
    status(seeded, "KON-1") == WorkflowStatus::Todo && status(seeded, "KON-11") == WorkflowStatus::Todo
  });
  assert_eq!((runs(&seeded, "KON-1"), runs(&seeded, "KON-11")), ran, "no run started");
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
    refusal(&mut seeded, "pipeline-none"),
    "Pipelines change on the task's page"
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
