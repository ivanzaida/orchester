//! The shared task list says where it lives (`Xr4Wb`, `aocsw`), and once the person is no longer a member it is
//! read-only with a notice that offers *Check again* and *Make local…* (`sErKV`); Sharing then says so too, with the
//! members as last read and *Make local* without *Leave* (`xLeTJ`). Its board is read-only too: cards neither lift nor
//! move or are selected by the keys or Shift+click, and quick add is locked, while More actions stays with Open and
//! Copy key (`15 Not a member` `kMKKv`).

use lurq::app::events::MouseButton;
use orchester_app::routes::{ProjectPage, Route, TaskList, TaskListTab};

use super::{KONTUR_REMOTE, harness, join, link_kontur, local_keys, local_tasks, no_admin_endpoint, server};
use crate::harness::{go, press, shows};

#[test]
fn shared_project_task_list_says_where_it_lives_and_when_it_is_read_only() {
  let (server, kontur) = server();
  let mut window = harness::window(Some(KONTUR_REMOTE));
  local_tasks(&window, &["Retry webhooks"]);
  let recorder = join(&window, &server, &kontur, ("ivan", "Ivan Zaida"));
  link_kontur(&window);
  let tasks = Route::Project(crate::support::project_id(), ProjectPage::Tasks(TaskList::default()));

  go(&mut window, tasks.clone(), "tasks-shared-line");
  assert_eq!(window.text("tasks-shared-server"), "Shared on Kontur team");
  window.until("the list synced", |window| {
    window
      .texts("tasks-shared-state")
      .is_some_and(|texts| texts.join(" ").starts_with("Synced "))
  });
  assert_eq!(window.text("tasks-shared-role"), "You're a member · ivan");
  assert!(window.tree.get_element_by_id("tasks-not-member").is_none());
  press(&mut window, "tasks-shared-settings");
  shows(&mut window, "sharing-members-count");

  // Removed in the server admin: the app learns it at its next read.
  server.remove_member(&kontur, "ivan");
  window.launch.services().expect("services").servers.sync_now();
  go(&mut window, tasks, "tasks-not-member");
  assert!(window.text("tasks-shared-state").starts_with("Not a member since "));
  assert_eq!(window.text("tasks-shared-role"), "You were a member · ivan");
  assert!(
    window
      .text("tasks-not-member")
      .contains("ask an owner, Alice Moreau, to add you again")
  );
  assert_eq!(
    window.text("tasks-new-task-reason-text"),
    "Read-only: you're not a member."
  );

  press(&mut window, "tasks-not-member-make-local");
  shows(&mut window, "sharing-make-local-question");
  shows(&mut window, "sharing-removed");
  assert_eq!(
    window.text("sharing-make-local-membership-value"),
    "You're no longer a member of KON"
  );
  assert!(
    window.tree.get_element_by_id("sharing-leave").is_none(),
    "nothing to leave"
  );
  shows(&mut window, "sharing-members-list");
  press(&mut window, "sharing-members-list");
  shows(&mut window, "sharing-member-ivan-hint");
  assert!(
    window.text("sharing-member-ivan-hint").contains("· you, until "),
    "{}",
    window.text("sharing-member-ivan-hint")
  );
  shows(&mut window, "sharing-members-come-back-row");
  no_admin_endpoint(&recorder);
}

#[test]
fn shared_project_task_list_without_tasks_keeps_its_line_and_locks_new_task() {
  let (server, kontur) = server();
  let mut window = harness::window(Some(KONTUR_REMOTE));
  let _recorder = join(&window, &server, &kontur, ("ivan", "Ivan Zaida"));
  link_kontur(&window);
  let tasks = Route::Project(crate::support::project_id(), ProjectPage::Tasks(TaskList::default()));

  go(&mut window, tasks.clone(), "tasks-empty");
  shows(&mut window, "tasks-shared-line");
  assert_eq!(window.text("tasks-shared-server"), "Shared on Kontur team");
  assert!(window.tree.get_element_by_id("tasks-empty-new-task-reason").is_none());

  server.remove_member(&kontur, "ivan");
  window.launch.services().expect("services").servers.sync_now();
  go(&mut window, tasks, "tasks-not-member");
  shows(&mut window, "tasks-empty");
  assert_eq!(window.text("tasks-shared-role"), "You were a member · ivan");
  assert_eq!(
    window.text("tasks-empty-new-task-reason-text"),
    "Read-only: you're not a member."
  );
}

#[test]
fn shared_project_board_of_a_former_member_is_read_only_with_more_actions_kept() {
  let (server, kontur) = server();
  let mut window = harness::window(Some(KONTUR_REMOTE));
  local_tasks(&window, &["Retry webhooks"]);
  let _recorder = join(&window, &server, &kontur, ("ivan", "Ivan Zaida"));
  link_kontur(&window);
  let tasks = Route::Project(
    crate::support::project_id(),
    ProjectPage::Tasks(TaskList::new(TaskListTab::All)),
  );
  server.remove_member(&kontur, "ivan");
  window.launch.services().expect("services").servers.sync_now();
  go(&mut window, tasks, "tasks-not-member");
  press(&mut window, "tasks-layout-board");
  shows(&mut window, "tasks-board-columns");
  let key = local_keys(&window).remove(0);
  let card = format!("task-card-{key}");
  window.until("the card laid out", |window| {
    window
      .tree
      .get_element_by_id_mut(&card)
      .and_then(|card| card.bounds())
      .is_some()
  });

  // Quick add is locked: Backlog keeps its foot, which opens no field.
  assert_eq!(window.text("tasks-board-new-task"), "New task");
  press(&mut window, "tasks-board-new-task");
  assert!(window.tree.get_element_by_id("tasks-board-quick-add-title").is_none());

  // The card does not lift: a drag onto Todo changes nothing.
  let bounds = |window: &mut crate::support::Window, id: &str| {
    window
      .tree
      .get_element_by_id_mut(id)
      .and_then(|element| element.bounds())
      .unwrap_or_else(|| panic!("{id} laid out"))
  };
  let (from, todo) = (
    bounds(&mut window, &card),
    bounds(&mut window, "tasks-board-column-todo"),
  );
  let (x0, y0) = (from.x + from.width / 2.0, from.y + from.height / 2.0);
  let (x1, y1) = (todo.x + todo.width / 2.0, todo.y + 80.0);
  window.tree.mouse_move(x0, y0);
  window.tree.mouse_down(x0, y0, MouseButton::Left);
  window.tree.mouse_move((x0 + x1) / 2.0, (y0 + y1) / 2.0);
  window.pass();
  assert!(
    window.tree.get_element_by_id("tasks-board-lifted").is_none(),
    "nothing lifts"
  );
  window.tree.mouse_move(x1, y1);
  window.tree.mouse_up(x1, y1, MouseButton::Left);
  window.pass();
  assert!(window.tree.get_element_by_id("tasks-board-hint").is_none());

  // More actions stays: the pointer over the card brings it up, with Open and Copy key and no change.
  window.tree.mouse_move(x0, y0);
  window.pass();
  shows(&mut window, &format!("task-more-{key}"));
  press(&mut window, &format!("task-more-{key}"));
  shows(&mut window, &format!("task-menu-{key}-copy"));
  for change in ["move", "priority", "tags", "done", "cancel", "delete"] {
    assert!(
      window
        .tree
        .get_element_by_id(&format!("task-menu-{key}-{change}"))
        .is_none(),
      "no {change} for a former member"
    );
  }
  key_down(&mut window, "Escape");
  neither_selected_nor_moved(&mut window, &key, (x0, y0));
}

/// Neither Shift+click on the card of `key` at (`x`, `y`) nor the board's keys select or move it.
fn neither_selected_nor_moved(window: &mut crate::support::Window, key: &str, (x, y): (f32, f32)) {
  // Shift+click selects nothing, so no selection bar offers changes.
  window.tree.mouse_move_with_meta(x, y, true, false, false, false);
  window
    .tree
    .mouse_down_with_meta(x, y, MouseButton::Left, true, false, false, false);
  window
    .tree
    .mouse_up_with_meta(x, y, MouseButton::Left, true, false, false, false);
  window.pass();
  assert!(
    window.tree.get_element_by_id("tasks-board-bulk").is_none(),
    "nothing selected"
  );

  // The keys reach the card (`.` opens its menu) but neither move nor select it, nor open a list to change it.
  key_down(window, "ArrowDown");
  key_down(window, ".");
  shows(window, &format!("task-menu-{key}-copy"));
  key_down(window, "Escape");
  let before = column_of(window, key);
  assert!(before.is_some(), "the card is in a column");
  for typed in ["]", "[", "x", "m"] {
    key_down(window, typed);
  }
  assert_eq!(column_of(window, key), before, "the keys moved nothing");
  assert!(
    window.tree.get_element_by_id("tasks-board-bulk").is_none(),
    "X selects nothing"
  );
  assert!(
    window
      .tree
      .get_element_by_id(&format!("task-menu-{key}-list"))
      .is_none(),
    "M opens no Move to"
  );
}

/// A key pressed without a modifier.
fn key_down(window: &mut crate::support::Window, key: &str) {
  window
    .tree
    .key_down(key.to_owned(), key.to_owned(), false, false, false);
  window.pass();
}

/// The board's status column that shows the card of `key`.
fn column_of(window: &mut crate::support::Window, key: &str) -> Option<&'static str> {
  ["backlog", "todo", "in_progress", "in_review", "done"]
    .into_iter()
    .find(|column| {
      window
        .texts(&format!("tasks-board-column-{column}"))
        .is_some_and(|texts| texts.iter().any(|text| text == key))
    })
}
