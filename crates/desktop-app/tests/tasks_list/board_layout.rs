//! Nothing on the Tasks board moves while a card is lifted or a question is open (ORC-105, the person's decision of
//! 2026-10-27): what refuses the card and Done's *Mark as done* and *Cancel task* are drawn over the columns' cards
//! and go with the drag (`03 Drop refused` `bR7zd`); a column that takes the card shows a line where it would go
//! (`02 Dragging` `goJvp`); the stop confirmation, Run again and *Delete KON-15?* are asked in a popover at the card,
//! the card waiting in its target column, and Esc, a press outside or the keep button put it back (`04 Confirm in
//! place` `ksvIz`). `cargo test -p orchester-app --test tasks_list tasks_board_layout_`.

use lurq::{app::events::MouseButton, core::ElementRect};
use orchester_app::routes::{TaskList, TaskListTab};
use orchester_core::task::organization::WorkflowStatus;
use orchester_store::ProjectDb;

use crate::{
  board::{in_column, press, refusal},
  harness::Seeded,
};

/// A question's width (`Popover · stop run` `XdfNz`).
const QUESTION_WIDTH: f32 = 340.0;

fn status(seeded: &Seeded, key: &str) -> WorkflowStatus {
  let db = ProjectDb::open(&seeded.repo).expect("the project");
  let task = orchester_core::store::TaskStore::tasks(&db)
    .expect("tasks")
    .into_iter()
    .find(|task| task.key().to_string() == key)
    .expect("the task");
  task.organization().status()
}

/// Every card's frame as laid out, but a card waiting for an answer in another column.
fn cards(seeded: &mut Seeded) -> Vec<(String, ElementRect)> {
  let ids: Vec<String> = seeded
    .ids("task-card-frame-")
    .into_iter()
    .filter(|id| !id.ends_with("-pending"))
    .collect();
  ids
    .into_iter()
    .map(|id| {
      let bounds = seeded.bounds(&id);
      (id, bounds)
    })
    .collect()
}

/// The cards of `now` that moved from where `before` had them, but those whose middle lies in `except`.
fn moved(before: &[(String, ElementRect)], now: &[(String, ElementRect)], except: Option<ElementRect>) -> Vec<String> {
  let same = |a: &ElementRect, b: &ElementRect| {
    (a.x - b.x).abs() < 0.5
      && (a.y - b.y).abs() < 0.5
      && (a.width - b.width).abs() < 0.5
      && (a.height - b.height).abs() < 0.5
  };
  before
    .iter()
    .filter(|(_, was)| {
      except.is_none_or(|column| {
        let middle = was.x + was.width / 2.0;
        middle < column.x || middle > column.x + column.width
      })
    })
    .filter(|(id, was)| !now.iter().any(|(held, is)| held == id && same(was, is)))
    .map(|(id, _)| id.clone())
    .collect()
}

/// Asserts that `cover` is laid out over the whole of `under`: a layer placed by `absolute` outside a stack has no
/// size, and draws nothing.
fn covers(seeded: &mut Seeded, under: &str, cover: &str) {
  let under_bounds = seeded.bounds(under);
  let cover_bounds = seeded.bounds(cover);
  assert!(
    under_bounds.width > 0.0 && under_bounds.height > 0.0,
    "{under} has a size: {under_bounds:?}"
  );
  for (is, should) in [
    (cover_bounds.x, under_bounds.x),
    (cover_bounds.y, under_bounds.y),
    (cover_bounds.width, under_bounds.width),
    (cover_bounds.height, under_bounds.height),
  ] {
    assert!(
      (is - should).abs() < 0.5,
      "{cover} covers {under}: {cover_bounds:?} over {under_bounds:?}"
    );
  }
}

fn escape(seeded: &mut Seeded) {
  seeded
    .window
    .tree
    .key_down("Escape".to_owned(), "Escape".to_owned(), false, false, false);
  seeded.window.pass();
}

/// Lets go of a held card at `(x, y)`.
fn release(seeded: &mut Seeded, (x, y): (f32, f32)) {
  seeded.window.tree.mouse_move(x, y);
  seeded.window.pass();
  seeded.window.tree.mouse_up(x, y, MouseButton::Left);
  seeded.window.pass();
}

#[test]
fn tasks_board_layout_a_refused_card_is_shown_over_the_columns_and_no_card_moves() {
  let mut seeded = Seeded::board("tasks", TaskList::new(TaskListTab::All));
  let before = cards(&mut seeded);
  // Held over In review, which refuses it: every column but its own and Done is dimmed under a chip, and Done offers
  // Mark as done and Cancel task over its cards (`03 Drop refused`).
  let (from, _) = seeded.hold("KON-12", "in_review");
  seeded.until("the overlays", |seeded| {
    seeded.laid_out("tasks-board-column-done-zone-done")
  });
  assert!(seeded.laid_out("tasks-board-column-done-zone-canceled"));
  // Done's zones not under the pointer are veiled over their whole size (`GdDT9`, `XzRhw`).
  for zone in ["done", "canceled"] {
    let id = format!("tasks-board-column-done-zone-{zone}");
    covers(&mut seeded, &id, &format!("{id}-veil"));
  }
  for column in ["backlog", "todo", "in_review"] {
    // The scrim dims the whole of the column's cards, not just a point of it (`wjtLk`).
    let overlay = format!("tasks-board-column-{column}-overlay");
    covers(&mut seeded, &overlay, &format!("tasks-board-column-{column}-scrim"));
    assert!(
      seeded.bounds(&overlay).height > 100.0,
      "{column}'s layer spans its cards"
    );
    assert_eq!(refusal(&mut seeded, column), "Run not ended: stop it from the task");
  }
  assert!(
    !seeded.shows("tasks-board-column-in_progress-overlay"),
    "the card's own column keeps its place, and nothing more"
  );
  assert_eq!(
    seeded.text("tasks-board-hint-text"),
    "KON-12 goes only to Done or Canceled"
  );
  let now = cards(&mut seeded);
  assert_eq!(moved(&before, &now, None), Vec::<String>::new(), "no card moved");

  // Let go where it was taken from: the overlays go with the drag, and nothing changes.
  release(&mut seeded, from);
  for id in [
    "tasks-board-column-done-overlay",
    "tasks-board-column-backlog-overlay",
    "tasks-board-column-in_review-refused",
  ] {
    assert!(!seeded.shows(id), "{id} went with the drag");
  }
  assert_eq!(status(&seeded, "KON-12"), WorkflowStatus::InProgress);
  assert!(
    seeded.shows("tasks-board"),
    "a card let go where it was taken opens nothing"
  );
  assert_eq!(moved(&before, &cards(&mut seeded), None), Vec::<String>::new());
}

#[test]
fn tasks_board_layout_a_column_that_takes_the_card_shows_a_line_and_no_card_moves() {
  let mut seeded = Seeded::board("tasks", TaskList::new(TaskListTab::All));
  let before = cards(&mut seeded);
  let (from, _) = seeded.hold("KON-15", "in_progress");
  seeded.until("the insertion line", |seeded| seeded.laid_out("tasks-board-insertion"));
  assert!(!seeded.shows("tasks-board-slot"), "no slot among the cards");
  let line = seeded.bounds("tasks-board-insertion");
  let column = seeded.bounds("tasks-board-column-in_progress");
  assert!((line.height - 2.0).abs() < 0.5, "a 2 high line: {line:?}");
  assert!(line.x >= column.x - 0.5 && line.x + line.width <= column.x + column.width + 0.5);
  assert_eq!(
    moved(&before, &cards(&mut seeded), None),
    Vec::<String>::new(),
    "no card moved"
  );
  release(&mut seeded, from);
  assert!(!seeded.shows("tasks-board-insertion"), "the line goes with the drag");
}

#[test]
fn tasks_board_layout_the_stop_question_is_asked_at_the_card_and_esc_puts_it_back() {
  let mut seeded = Seeded::board("tasks", TaskList::new(TaskListTab::All));
  let columns = seeded.bounds("tasks-board-columns");
  let before = cards(&mut seeded);
  seeded.drag("KON-12", "done");
  seeded.until("the stop question", |seeded| {
    seeded.laid_out("tasks-close-stop-confirm")
  });
  let asked = seeded.bounds("tasks-close-stop");
  assert!((asked.width - QUESTION_WIDTH).abs() < 0.5, "a popover: {asked:?}");
  assert_eq!(seeded.text("tasks-close-stop-confirm"), "Stop run and mark as done");
  // The card waits at Done's top, the popover under it; its place in In progress is kept.
  let waiting = seeded.bounds("task-card-KON-12-pending");
  let done = seeded.bounds("tasks-board-column-done");
  assert!(waiting.x >= done.x - 0.5 && waiting.x + waiting.width <= done.x + done.width + 0.5);
  assert!(asked.y >= waiting.y + waiting.height, "under the card");
  assert!(
    (asked.x + asked.width - (done.x + done.width)).abs() < 1.0,
    "its right edge on the column's: {asked:?} {done:?}"
  );
  assert_eq!(
    seeded.bounds("tasks-board-columns").y,
    columns.y,
    "nothing pushed the columns"
  );
  assert_eq!(
    moved(&before, &cards(&mut seeded), Some(done)),
    Vec::<String>::new(),
    "no card outside Done moved"
  );

  escape(&mut seeded);
  assert!(!seeded.shows("tasks-close-stop"), "Esc closed the question");
  assert!(
    !seeded.shows("task-card-KON-12-pending"),
    "the card is back in its place"
  );
  assert!(in_column(&mut seeded, "KON-12", "in_progress"));
  assert_eq!(status(&seeded, "KON-12"), WorkflowStatus::InProgress);
  assert_eq!(moved(&before, &cards(&mut seeded), None), Vec::<String>::new());
}

#[test]
fn tasks_board_layout_enter_confirms_the_stop_and_its_failure_is_said_in_the_popover() {
  let mut seeded = Seeded::board("tasks", TaskList::new(TaskListTab::All));
  seeded.drag("KON-12", "done");
  seeded.until("the stop question", |seeded| {
    seeded.laid_out("tasks-close-stop-confirm")
  });
  seeded
    .window
    .tree
    .key_down("Enter".to_owned(), "Enter".to_owned(), false, false, false);
  seeded.window.pass();
  // No runner serves the project: the stop is asked of it and fails, said under the question, which stays.
  seeded.until("why it failed", |seeded| seeded.laid_out("tasks-close-stop-error"));
  let said = seeded.text("tasks-close-stop-error");
  assert!(said.contains("runner"), "{said}");
  assert!(seeded.laid_out("task-card-KON-12-pending"), "the card still waits");
  assert_eq!(status(&seeded, "KON-12"), WorkflowStatus::InProgress);
}

#[test]
fn tasks_board_layout_a_press_outside_the_question_keeps_the_task_and_cancel_asks_its_own_words() {
  let mut seeded = Seeded::board("tasks", TaskList::new(TaskListTab::All));
  // Held over Done's *Cancel task* zone and let go there (`JUaF2`).
  let (_, at) = seeded.hold("KON-12", "done");
  seeded.until("Done's zones", |seeded| {
    seeded.laid_out("tasks-board-column-done-zone-canceled")
  });
  let zone = seeded.bounds("tasks-board-column-done-zone-canceled");
  let middle = (zone.x + zone.width / 2.0, zone.y + zone.height / 2.0);
  seeded.window.tree.mouse_move(at.0, (at.1 + middle.1) / 2.0);
  seeded.window.pass();
  release(&mut seeded, middle);
  seeded.until("the stop question", |seeded| {
    seeded.laid_out("tasks-close-stop-confirm")
  });
  assert_eq!(seeded.text("tasks-close-stop-confirm"), "Stop run and cancel task");
  // A press outside closes it; nothing changes.
  let head = seeded.bounds("tasks-board-column-backlog-name");
  let (x, y) = (head.x + head.width / 2.0, head.y + head.height / 2.0);
  seeded.window.tree.mouse_move(x, y);
  seeded.window.tree.mouse_down(x, y, MouseButton::Left);
  seeded.window.tree.mouse_up(x, y, MouseButton::Left);
  seeded.window.pass();
  seeded.until("the question closed", |seeded| !seeded.shows("tasks-close-stop"));
  assert!(!seeded.shows("task-card-KON-12-pending"));
  assert_eq!(status(&seeded, "KON-12"), WorkflowStatus::InProgress);
}

#[test]
fn tasks_board_layout_run_again_and_delete_ask_in_a_popover_at_the_card() {
  let mut seeded = Seeded::board("tasks", TaskList::new(TaskListTab::All));
  let columns = seeded.bounds("tasks-board-columns");
  let before = cards(&mut seeded);
  seeded.drag("KON-1", "todo");
  seeded.until("the question", |seeded| seeded.laid_out("tasks-board-ask-keep"));
  let asked = seeded.bounds("tasks-board-ask");
  assert!((asked.width - QUESTION_WIDTH).abs() < 0.5, "a popover: {asked:?}");
  assert!(seeded.laid_out("task-card-KON-1-pending"), "KON-1 waits in Todo");
  let todo = seeded.bounds("tasks-board-column-todo");
  assert_eq!(seeded.bounds("tasks-board-columns").y, columns.y);
  assert_eq!(moved(&before, &cards(&mut seeded), Some(todo)), Vec::<String>::new());
  escape(&mut seeded);
  assert!(!seeded.shows("tasks-board-ask"));
  assert_eq!(status(&seeded, "KON-1"), WorkflowStatus::Done, "nothing changed");

  // *Delete KON-15?* is asked at the card where it is.
  seeded.hover("task-card-KON-15");
  press(&mut seeded, "task-more-KON-15");
  press(&mut seeded, "task-menu-KON-15-delete");
  seeded.until("the delete question", |seeded| {
    seeded.laid_out("tasks-close-delete-title")
  });
  let asked = seeded.bounds("tasks-close-delete");
  assert!((asked.width - QUESTION_WIDTH).abs() < 0.5, "a popover: {asked:?}");
  assert_eq!(seeded.text("tasks-close-delete-title"), "Delete KON-15?");
  assert_eq!(seeded.bounds("tasks-board-columns").y, columns.y);
  assert_eq!(
    moved(&before, &cards(&mut seeded), None),
    Vec::<String>::new(),
    "no card moved"
  );
  escape(&mut seeded);
  assert!(!seeded.shows("tasks-close-delete"));
  assert!(in_column(&mut seeded, "KON-15", "todo"));

  // At a card of the first column, Backlog, it opens from the card's start, clear of the sidebar.
  seeded.hover("task-card-KON-3");
  press(&mut seeded, "task-more-KON-3");
  press(&mut seeded, "task-menu-KON-3-delete");
  seeded.until("the delete question", |seeded| {
    seeded.laid_out("tasks-close-delete-title")
  });
  let (asked, card) = (seeded.bounds("tasks-close-delete"), seeded.bounds("task-card-KON-3"));
  assert!(
    (asked.x - card.x).abs() < 0.5,
    "from the card's start: {asked:?} {card:?}"
  );
  assert!(asked.x >= columns.x, "inside the board: {asked:?} {columns:?}");
  escape(&mut seeded);
}

fn enter(seeded: &mut Seeded) {
  seeded
    .window
    .tree
    .key_down("Enter".to_owned(), "Enter".to_owned(), false, false, false);
  seeded.window.pass();
}

fn stored(seeded: &Seeded, key: &str) -> bool {
  let db = ProjectDb::open(&seeded.repo).expect("the project");
  orchester_core::store::TaskStore::tasks(&db)
    .expect("tasks")
    .iter()
    .any(|task| task.key().to_string() == key)
}

#[test]
fn tasks_board_layout_enter_never_confirms_a_deletion() {
  let mut seeded = Seeded::board("tasks", TaskList::new(TaskListTab::All));
  // Several selected: *Delete 2 tasks?* waits for its button.
  seeded.shift_click("task-card-KON-3");
  seeded.shift_click("task-card-KON-15");
  press(&mut seeded, "tasks-board-bulk-delete");
  seeded.until("the delete question", |seeded| seeded.laid_out("tasks-board-ask-keep"));
  enter(&mut seeded);
  assert!(seeded.laid_out("tasks-board-ask"), "the question stays");
  assert!(stored(&seeded, "KON-3") && stored(&seeded, "KON-15"), "nothing deleted");
  escape(&mut seeded);
  assert!(!seeded.shows("tasks-board-ask"));
  escape(&mut seeded);

  // One card's *Delete KON-15?*: Enter neither deletes nor opens the focused card's task.
  seeded.hover("task-card-KON-15");
  press(&mut seeded, "task-more-KON-15");
  press(&mut seeded, "task-menu-KON-15-delete");
  seeded.until("the delete question", |seeded| {
    seeded.laid_out("tasks-close-delete-title")
  });
  enter(&mut seeded);
  assert!(seeded.laid_out("tasks-close-delete"), "the question stays");
  assert!(seeded.laid_out("tasks-board-columns"), "still on the board");
  assert!(stored(&seeded, "KON-15"));
  escape(&mut seeded);
}
