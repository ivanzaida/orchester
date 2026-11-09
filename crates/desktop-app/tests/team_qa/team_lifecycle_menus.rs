//! A team task's lifecycle from the Tasks list's row menu and Mark as blocked's dialog (decision D4, `Task - v2`
//! `jDN0r` T7 `lCoNt`, T8 `vdSWS`, T10 `tlw0r`, T13 `DWfta`, T14 `uKVK2`, T15 `ahGVl`), on the QA app's `team-member`,
//! `team-admin` and `team-claims` states with the stand-in team server.
//!
//! `cargo test -p orchester-app --test team_qa team_qa_lifecycle_menu`

use lurq::app::events::with_synthetic_input;
use orchester_api::test_support::team_server::TeamServer;
use orchester_app::{
  routes::{ProjectPage, Route, TaskList, TaskListTab},
  test_support::{Window, project_id},
};

use super::{
  prepared,
  team::State,
  team_lifecycle::{at, offered, on},
};

/// The Tasks list on `state`, with `key`'s row menu open (T7 `lCoNt`, T8 `vdSWS`, T10 `tlw0r`).
fn row_menu(state: State, key: &str) -> (Window, TeamServer) {
  // The list's two lines under the header leave room for the first rows of a view only: a task is read in the view
  // that holds it near the top.
  let tab = if key == "KON-25" {
    TaskListTab::Backlog
  } else {
    TaskListTab::Active
  };
  row_menu_in(state, tab, key)
}

/// [`row_menu`] on the list's `tab`, where a task far down the list has its row.
fn row_menu_in(state: State, tab: TaskListTab, key: &str) -> (Window, TeamServer) {
  let (mut window, server) = prepared(state);
  window
    .navigation()
    .go(Route::Project(project_id(), ProjectPage::Tasks(TaskList::new(tab))));
  let row = format!("task-row-{key}");
  window.until("the server's tasks", |window| window.texts(&row).is_some());
  // The row's actions show under the pointer once the row has its place, which can take a pass after it shows.
  let more = format!("task-more-{key}");
  window.until("the row's menu", |window| {
    let placed = window
      .tree
      .get_element_by_id_mut(&row)
      .is_some_and(|row| row.bounds().is_some());
    if placed {
      window.hover(&row);
    }
    window.texts(&more).is_some()
  });
  window.press(&more);
  window.until("its rows", |window| window.texts(&format!("{more}-panel")).is_some());
  (window, server)
}

/// T7 `lCoNt`: a member's row menu on their own task offers Mark as done and Mark as blocked and says who does the
/// rest, with the keys of Open and Copy key at their rows' ends.
#[test]
fn team_qa_lifecycle_menu_a_members_row_menu_offers_what_their_claim_allows() {
  let (mut window, _server) = row_menu(State::Member, "KON-12");
  for id in ["open", "copy", "done", "block"] {
    assert!(offered(&mut window, &format!("task-more-KON-12-{id}")), "{id}");
  }
  for id in ["cancel", "reopen", "move", "delete"] {
    assert!(!offered(&mut window, &format!("task-more-KON-12-{id}")), "{id}");
  }
  assert!(
    window
      .text("task-more-KON-12-open")
      .split_whitespace()
      .any(|word| word == "Enter"),
    "Open ends in its key: {}",
    window.text("task-more-KON-12-open")
  );
  assert_eq!(
    window.text("tasks-team-menu-foot"),
    "Only Admins cancel, reopen or move tasks."
  );
  window.press("task-more-KON-12-done");
  window.until("the server to close it", |window| {
    window.texts("task-row-KON-12").is_none() || window.texts("tasks-team-refused").is_some()
  });
  assert!(
    window.texts("tasks-team-refused").is_none(),
    "the holder's Done is taken"
  );
}

#[test]
fn team_qa_lifecycle_menu_a_members_row_menu_on_someone_elses_task_only_opens_and_copies() {
  let (mut window, _server) = row_menu(State::Claims, "KON-13");
  assert!(offered(&mut window, "task-more-KON-13-open") && offered(&mut window, "task-more-KON-13-copy"));
  for id in ["done", "block", "cancel", "reopen", "move"] {
    assert!(!offered(&mut window, &format!("task-more-KON-13-{id}")), "{id}");
  }
  window.until("the Admins' names", |window| {
    window
      .texts("tasks-team-menu-foot")
      .is_some_and(|foot| foot.concat().contains("dana-k"))
  });
  assert_eq!(
    window.text("tasks-team-menu-foot"),
    "Only mara (the claim) and Admins close this task: dana-k and olha."
  );
}

/// T8 `vdSWS`: an Admin's row menu has Mark as done, Mark as blocked and Cancel task, and no Move to for a task that is
/// in progress (the server allows it from Backlog and Todo only), with no line saying who else can; Cancel task closes
/// it on the server.
#[test]
fn team_qa_lifecycle_menu_an_admins_row_menu_closes_and_cancels() {
  let (mut window, _server) = row_menu(State::Admin, "KON-12");
  for id in ["open", "copy", "done", "block", "cancel"] {
    assert!(offered(&mut window, &format!("task-more-KON-12-{id}")), "{id}");
  }
  assert!(
    !offered(&mut window, "tasks-team-menu-foot"),
    "an Admin has nothing to be told"
  );
  window.press("task-more-KON-12-cancel");
  window.until("the task to leave the active list", |window| {
    window.texts("task-row-KON-12").is_none()
  });
}

/// T10 `tlw0r`: a Backlog task's *Move to* opens a list of Backlog and Todo, the current one checked, with its foot; a
/// choice moves the task on the server.
#[test]
fn team_qa_lifecycle_menu_move_to_lists_backlog_and_todo() {
  let (mut window, server) = row_menu(State::Admin, "KON-25");
  assert!(offered(&mut window, "task-more-KON-25-move"));
  assert!(
    !offered(&mut window, "task-more-KON-25-reopen"),
    "an open task has no Reopen task"
  );
  window.press("task-more-KON-25-move");
  window.until("Move to's list", |window| {
    window.texts("task-more-KON-25-list").is_some()
  });
  assert!(offered(&mut window, "task-more-KON-25-move-backlog"));
  assert_eq!(
    window.text("task-more-KON-25-list-foot"),
    "Runs move a task on from there."
  );
  window.press("task-more-KON-25-move-todo");
  window.until("the server to take the move", |_| {
    server.task_now(25).is_some_and(|(revision, ..)| revision == 2)
  });
}

/// T10 `tlw0r`: a Done task's menu offers *Reopen task*, whose list is Back to Backlog and Back to Todo with its foot;
/// it offers no Mark as done, Mark as blocked or Cancel task.
#[test]
fn team_qa_lifecycle_menu_reopen_task_lists_where_the_task_goes_back_to() {
  let (mut window, server) = row_menu_in(State::Admin, TaskListTab::Done, "KON-1");
  assert!(offered(&mut window, "task-more-KON-1-reopen"));
  for id in ["done", "block", "cancel", "move"] {
    assert!(
      !offered(&mut window, &format!("task-more-KON-1-{id}")),
      "{id} on a Done task"
    );
  }
  window.press("task-more-KON-1-reopen");
  window.until("Reopen's list", |window| window.texts("task-more-KON-1-list").is_some());
  assert!(offered(&mut window, "task-more-KON-1-reopen-backlog"));
  assert_eq!(
    window.text("task-more-KON-1-list-foot"),
    "A reopened task is free: nobody holds it."
  );
  window.press("task-more-KON-1-reopen-todo");
  window.until("the server to take the reopening", |_| {
    server.task_now(1).is_some_and(|(revision, ..)| revision == 2)
  });
}

/// A closed task reads closed in the list, whatever its runs never said: Done and Canceled in their own words, not
/// *Ready to queue*.
#[test]
fn team_qa_lifecycle_menu_closed_tasks_read_closed_in_the_list() {
  let (mut window, _server) = prepared(State::AdminWork);
  window.navigation().go(Route::Project(
    project_id(),
    ProjectPage::Tasks(TaskList::new(TaskListTab::Done)),
  ));
  window.until("the Done tab", |window| window.texts("task-row-KON-1").is_some());
  assert_eq!(window.text("task-state-KON-1"), "Done");
  assert_eq!(window.text("task-state-KON-15"), "Canceled");
  for key in ["KON-1", "KON-15"] {
    assert!(
      !window.text(&format!("task-row-{key}")).contains("Ready to queue"),
      "{key} is not ready to queue"
    );
  }
}

/// A member's menu on a Done task: only Open and Copy key, and the line saying only Admins reopen it.
#[test]
fn team_qa_lifecycle_menu_a_members_menu_on_a_done_task_offers_no_block() {
  let (mut window, _server) = row_menu_in(State::Member, TaskListTab::Done, "KON-1");
  for id in ["done", "block", "cancel", "reopen"] {
    assert!(
      !offered(&mut window, &format!("task-more-KON-1-{id}")),
      "{id} on a Done task"
    );
  }
  assert!(
    window
      .text("tasks-team-menu-foot")
      .starts_with("Only Admins reopen a team task")
  );
}

/// The page's Mark as blocked dialog, open on KON-12 as its holder reads it (T13 `DWfta`).
fn dialog() -> Window {
  let (mut window, _server) = on(State::Member, "KON-12");
  window.press("task-team-block");
  window.until("the reason's question", |window| {
    window.texts("task-block-reason").is_some()
  });
  window
}

/// T13 `DWfta`, T14 `uKVK2`, T15 `ahGVl`: the dialog names the task and asks for the reason under its label with its
/// hint; while the field is empty or holds only spaces the primary is disabled and the footer says why, and once text
/// is typed the footer says *Esc cancels*.
#[test]
fn team_qa_lifecycle_menu_the_blocking_dialog_waits_for_a_reason() {
  let mut window = dialog();
  assert_eq!(window.text("task-block-title"), "Mark KON-12 as blocked");
  assert_eq!(window.text("task-block-label"), "Why is it blocked?");
  assert_eq!(
    window.text("task-block-hint"),
    "Required. It shows on the task's rail and in the Tasks list."
  );
  assert_eq!(
    window.text("task-block-unavailable-text"),
    "Unavailable: say what it waits for."
  );
  window.press("task-block-confirm");
  assert!(
    offered(&mut window, "task-block-reason") && !offered(&mut window, "task-team-blocked-title"),
    "the disabled primary does nothing"
  );

  window.set("task-block-reason", "   ");
  assert!(
    offered(&mut window, "task-block-unavailable"),
    "spaces alone are no reason"
  );
  window.press("task-block-confirm");
  assert!(offered(&mut window, "task-block-reason"));

  window.set("task-block-reason", "Waiting for the staging API key");
  assert!(!offered(&mut window, "task-block-unavailable"));
  assert_eq!(window.text("task-block-keys"), "Esc cancels");
}

/// T15 `ahGVl` 04: *Cancel*, Esc and a press on the dimmed page cancel the dialog and change nothing; it opens empty
/// again.
#[test]
fn team_qa_lifecycle_menu_the_blocking_dialog_cancels_three_ways() {
  let mut window = dialog();
  window.set("task-block-reason", "Waiting for the staging API key");
  window.press("task-block-cancel");
  assert!(!offered(&mut window, "task-block-reason"), "Cancel closes it");
  assert!(
    !offered(&mut window, "task-team-blocked-title"),
    "and the task is not blocked"
  );

  window.press("task-team-block");
  window.until("the question again", |window| {
    window.texts("task-block-reason").is_some()
  });
  assert!(
    offered(&mut window, "task-block-unavailable"),
    "it opens empty, as a first time"
  );
  with_synthetic_input(|| {
    window
      .tree
      .key_down("Escape".to_owned(), "Escape".to_owned(), false, false, false);
    window
      .tree
      .key_up("Escape".to_owned(), "Escape".to_owned(), false, false, false);
  });
  window.pass();
  assert!(!offered(&mut window, "task-block-reason"), "Esc closes it");

  window.press("task-team-block");
  window.until("the question once more", |window| {
    window.texts("task-block-reason").is_some()
  });
  let (x, y) = at(&mut window, "task-block-scrim");
  window.tree.mouse_move(x + 4.0, y + 4.0);
  window
    .tree
    .mouse_down(x + 4.0, y + 4.0, lurq::app::events::MouseButton::Left);
  window
    .tree
    .mouse_up(x + 4.0, y + 4.0, lurq::app::events::MouseButton::Left);
  window.pass();
  assert!(
    !offered(&mut window, "task-block-reason"),
    "a press on the page closes it"
  );
  assert!(!offered(&mut window, "task-team-blocked-title"));
}

/// The Tasks list's dialog is the page's: the same words and the same wait for a reason.
#[test]
fn team_qa_lifecycle_menu_the_list_asks_for_the_reason_in_the_same_dialog() {
  let (mut window, _server) = row_menu(State::Member, "KON-12");
  window.press("task-more-KON-12-block");
  window.until("the reason's question", |window| {
    window.texts("tasks-team-block-reason").is_some()
  });
  assert_eq!(window.text("tasks-team-block-title"), "Mark KON-12 as blocked");
  assert_eq!(window.text("tasks-team-block-label"), "Why is it blocked?");
  assert_eq!(
    window.text("tasks-team-block-unavailable-text"),
    "Unavailable: say what it waits for."
  );
  window.set("tasks-team-block-reason", "Waiting for the staging API key");
  assert_eq!(window.text("tasks-team-block-keys"), "Esc cancels");
}

/// Opens the row menu of `key` again once its first closed, the pointer back on the row.
fn reopen_menu(window: &mut Window, key: &str) {
  let (row, more) = (format!("task-row-{key}"), format!("task-more-{key}"));
  window.until("the menu to close", |window| !offered(window, &format!("{more}-panel")));
  window.until("the row's button", |window| {
    window.hover(&row);
    offered(window, &more)
  });
  window.press(&more);
  window.until("its rows", |window| offered(window, &format!("{more}-panel")));
}

fn press_escape(window: &mut Window) {
  with_synthetic_input(|| {
    window
      .tree
      .key_down("Escape".to_owned(), "Escape".to_owned(), false, false, false);
    window
      .tree
      .key_up("Escape".to_owned(), "Escape".to_owned(), false, false, false);
  });
  window.pass();
}

/// A menu that closed with its list open opens again as the plain menu, whichever way it closed: a choice in the list,
/// Esc, or its button.
#[test]
fn team_qa_lifecycle_menu_move_to_list_does_not_outlast_its_menu() {
  let (mut window, _server) = row_menu(State::Admin, "KON-25");
  let list = "task-more-KON-25-list";
  window.press("task-more-KON-25-move");
  window.until("Move to's list", |window| offered(window, list));
  // Backlog is the task's own status: a choice that changes nothing still closes the menu.
  window.press("task-more-KON-25-move-backlog");
  reopen_menu(&mut window, "KON-25");
  assert!(!offered(&mut window, list), "after a choice in Move to");
  assert!(offered(&mut window, "task-more-KON-25-move"));

  window.press("task-more-KON-25-move");
  window.until("Move to's list again", |window| offered(window, list));
  press_escape(&mut window);
  assert!(!offered(&mut window, list), "the first Esc closes the list");
  assert!(offered(&mut window, "task-more-KON-25-panel"), "and keeps the menu");
  press_escape(&mut window);
  assert!(
    !offered(&mut window, "task-more-KON-25-panel"),
    "the second closes the menu"
  );
  reopen_menu(&mut window, "KON-25");
  assert!(!offered(&mut window, list), "after Esc");

  window.press("task-more-KON-25-move");
  window.until("Move to's list once more", |window| offered(window, list));
  window.press("task-more-KON-25");
  reopen_menu(&mut window, "KON-25");
  assert!(!offered(&mut window, list), "after the button");
}

/// The same for a moving choice, which the server takes, and for *Reopen task* on a Done task.
#[test]
fn team_qa_lifecycle_menu_reopen_list_does_not_outlast_its_menu() {
  let (mut window, server) = row_menu(State::Admin, "KON-25");
  window.press("task-more-KON-25-move");
  window.until("Move to's list", |window| offered(window, "task-more-KON-25-list"));
  window.press("task-more-KON-25-move-todo");
  window.until("the server to take the move", |_| {
    server.task_now(25).is_some_and(|(revision, ..)| revision == 2)
  });
  reopen_menu(&mut window, "KON-25");
  assert!(
    !offered(&mut window, "task-more-KON-25-list"),
    "after a moving choice in Move to"
  );

  let (mut window, _server) = row_menu_in(State::Admin, TaskListTab::Done, "KON-1");
  window.press("task-more-KON-1-reopen");
  window.until("Reopen's list", |window| offered(window, "task-more-KON-1-list"));
  press_escape(&mut window);
  press_escape(&mut window);
  reopen_menu(&mut window, "KON-1");
  assert!(
    !offered(&mut window, "task-more-KON-1-list"),
    "after Esc in Reopen task"
  );
  assert!(offered(&mut window, "task-more-KON-1-reopen"));
}
