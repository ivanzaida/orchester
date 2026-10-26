//! What a shared task's runs deliver (ORC-93) in Edit task: the project's server does not keep it, so the Delivers chip
//! shows what the pipeline says with a lock, its popover says why it cannot be changed here, and a save never sends it.

use orchester_app::routes::{ProjectPage, Route};
use orchester_core::project::TaskKey;

use super::{KONTUR_REMOTE, harness, join, link_kontur, local_tasks, server};
use crate::harness::{go, press};

/// The server's reason, in the workspace's words (`sharing.errors.serverOnly`).
const SERVER_ONLY: &str =
  "This change is made on the server for a shared project, and this screen cannot send it there yet.";

#[test]
fn shared_project_delivers_is_shown_unavailable_and_a_save_never_sends_it() {
  let (server, kontur) = server();
  let mut window = harness::window(Some(KONTUR_REMOTE));
  local_tasks(&window, &["Retry webhooks"]);
  join(&window, &server, &kontur, ("ivan", "Ivan Zaida"));
  link_kontur(&window);

  let key: TaskKey = "KON-1".parse().expect("a key");
  go(
    &mut window,
    Route::Project(crate::support::project_id(), ProjectPage::EditTask(key)),
    "task-form-delivers",
  );
  assert!(
    window.tree.get_element_by_id("task-form-delivers-lock").is_some(),
    "a lock in place of a choice"
  );
  assert_eq!(window.text("task-form-delivers-value"), "As the pipeline says");
  press(&mut window, "task-form-delivers");
  window.until("the reason", |window| {
    window.tree.get_element_by_id("task-form-delivers-reason").is_some()
  });
  assert_eq!(window.text("task-form-delivers-reason"), SERVER_ONLY);
  assert!(
    window.tree.get_element_by_id("task-form-delivers-menu").is_none(),
    "no choice to make"
  );

  press(&mut window, "task-form-delivers");
  window.until("the popover closes", |window| {
    window.tree.get_element_by_id("task-form-delivers-reason").is_none()
  });
  window.set("task-form-title-input", "Retry webhooks with backoff");
  press(&mut window, "task-form-save");
  let shown = |window: &mut crate::support::Window| {
    (
      window.texts("task-form-status-label").unwrap_or_default().join(""),
      window.texts("task-form-reason-text").unwrap_or_default().join(""),
    )
  };
  let started = std::time::Instant::now();
  let (status, reason) = loop {
    window.tree.tick_futures();
    window.pass();
    let (status, reason) = shown(&mut window);
    let busy = status == "Unsaved changes" || status == "Saving changes…";
    if !busy || started.elapsed() > std::time::Duration::from_secs(20) {
      break (status, reason);
    }
    std::thread::sleep(std::time::Duration::from_millis(20));
  };
  assert!(!reason.contains(SERVER_ONLY), "the save sent no delivers: {reason}");
  assert_eq!(status, "Draft saved locally", "{reason}");
}
