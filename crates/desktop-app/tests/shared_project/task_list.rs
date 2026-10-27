//! The shared task list says where it lives (`Xr4Wb`, `aocsw`), and once the person is no longer a member it is
//! read-only with a notice that offers *Check again* and *Make local…* (`sErKV`); Sharing then says so too, with the
//! members as last read and *Make local* without *Leave* (`xLeTJ`). Its board is read-only too: cards do not lift and
//! quick add is locked, while More actions stays (`15 Not a member` `kMKKv`).

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
  shows(&mut window, "sharing-members-intro-text");

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
  shows(&mut window, "sharing-member-ivan-you");
  assert!(window.text("sharing-member-ivan-you").starts_with("· you, until "));
  shows(&mut window, "sharing-members-come-back");
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

  // More actions stays: the pointer over the card brings it up.
  window.tree.mouse_move(x0, y0);
  window.pass();
  shows(&mut window, &format!("task-more-{key}"));
}
