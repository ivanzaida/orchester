//! The shared task list says where it lives (`Xr4Wb`, `aocsw`), and once the person is no longer a member it is
//! read-only with a notice that offers *Check again* and *Make local…* (`sErKV`); Sharing then says so too, with the
//! members as last read and *Make local* without *Leave* (`xLeTJ`).

use orchester_app::routes::{ProjectPage, Route, TaskList};

use super::{KONTUR_REMOTE, harness, join, link_kontur, local_tasks, no_admin_endpoint, server};
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
    window.text("tasks-create-multiple-reason"),
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
