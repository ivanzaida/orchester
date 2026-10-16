//! A conflicting edit of a shared task (`PenUu`): Alice saved a newer title from her computer while the person
//! edited it here; the form shows the fields that differ side by side, named after Alice as the server names her.

use orchester_app::routes::{ProjectPage, Route};
use orchester_core::project::TaskKey;

use super::{KONTUR_REMOTE, harness, join, link_kontur, local_tasks, no_admin_endpoint, server};
use crate::harness::{go, press};

#[test]
fn shared_project_conflicting_edit_shows_the_fields_that_differ() {
  let (server, kontur) = server();
  let mut window = harness::window(Some(KONTUR_REMOTE));
  local_tasks(&window, &["Retry webhooks"]);
  let recorder = join(&window, &server, &kontur, ("ivan", "Ivan Zaida"));
  link_kontur(&window);

  let key: TaskKey = "KON-1".parse().expect("a key");
  go(
    &mut window,
    Route::Project(crate::support::project_id(), ProjectPage::EditTask(key)),
    "task-form-title-input",
  );
  window.set("task-form-title-input", "Retry webhooks with backoff");
  server.retitle(&kontur, "KON-1", "Retry webhooks three times");
  window.launch.services().expect("services").servers.sync_now();
  window.until("the newer version met, or Save changes", |window| {
    window.tree.get_element_by_id("task-form-conflict-fields").is_some()
      || window.tree.get_element_by_id("task-form-save").is_some()
  });
  if window.tree.get_element_by_id("task-form-conflict-fields").is_none() {
    press(&mut window, "task-form-save");
  }
  window.until("the fields to choose", |window| {
    window.tree.get_element_by_id("task-form-conflict-fields").is_some()
  });
  assert_eq!(
    window.text("task-form-conflict-notice-title"),
    "Alice Moreau saved a newer version of KON-1 while you were editing"
  );
  assert_eq!(
    window.text("task-form-conflict-title-mine-value"),
    "Retry webhooks with backoff"
  );
  assert_eq!(
    window.text("task-form-conflict-title-theirs-value"),
    "Retry webhooks three times"
  );
  assert!(
    window
      .text("task-form-conflict-title-theirs")
      .contains("Alice Moreau's version")
  );
  no_admin_endpoint(&recorder);
}
