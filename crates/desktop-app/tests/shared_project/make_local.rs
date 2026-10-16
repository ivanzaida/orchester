//! *Make local* keeps every task (`woioY`): it asks in place under its row with what happens to the tasks, the
//! person's claims and the membership; only a person confirms it; then the project is local with every task and its
//! number, and the page says so. While the server can't be reached it says the claims go stale instead and offers
//! *Make local anyway*. The tasks a join kept *Not shared* come back with numbers after the highest, and the page says
//! which.

use lurq::app::events::with_synthetic_input;
use orchester_core::id::IdempotencyKey;

use super::{
  KONTUR_REMOTE, entry, harness, join, kontur_root, link_kontur, local_keys, local_tasks, no_admin_endpoint,
  open_sharing, server,
};
use crate::harness::{block_on, press, shows};

#[test]
fn shared_project_make_local_keeps_every_task() {
  let (server, kontur) = server();
  let mut window = harness::window(Some(KONTUR_REMOTE));
  local_tasks(&window, &["Retry webhooks", "Show delivery status", "Paginate the log"]);
  let recorder = join(&window, &server, &kontur, ("ivan", "Ivan Zaida"));
  link_kontur(&window);

  open_sharing(&mut window, "sharing-make-local-button");
  press(&mut window, "sharing-make-local-button");
  shows(&mut window, "sharing-make-local-question");
  assert_eq!(
    window.text("sharing-make-local-tasks-value"),
    "All 3 stay as local tasks with their numbers"
  );
  assert_eq!(
    window.text("sharing-make-local-membership-value"),
    "You stay a member of KON"
  );

  with_synthetic_input(|| window.press("sharing-make-local-confirm"));
  shows(&mut window, "sharing-make-local-confirm-person-only");
  assert!(
    window.tree.get_element_by_id("sharing-made-local").is_none(),
    "an MCP client's press does nothing"
  );

  press(&mut window, "sharing-make-local-confirm");
  shows(&mut window, "sharing-made-local");
  shows(&mut window, "sharing-local");
  assert_eq!(local_keys(&window), ["KON-1", "KON-2", "KON-3"]);
  assert_eq!(server.task_keys(&kontur).len(), 3, "the server project is unchanged");
  press(&mut window, "sharing-made-local-dismiss");
  window.until("the notice put away", |window| {
    window.tree.get_element_by_id("sharing-made-local").is_none()
  });
  no_admin_endpoint(&recorder);
}

#[test]
fn shared_project_make_local_without_the_server_says_the_claims_go_stale() {
  let (server, kontur) = server();
  let mut window = harness::window(Some(KONTUR_REMOTE));
  local_tasks(&window, &["Retry webhooks", "Show delivery status"]);
  let recorder = join(&window, &server, &kontur, ("ivan", "Ivan Zaida"));
  link_kontur(&window);
  recorder.go_down();
  let services = window.launch.services().expect("services").clone();
  let server_id = entry(&window).stored.id;
  services.servers.retry_now(server_id);
  window.until("the server offline", |_| {
    !matches!(
      services.servers.connection(server_id),
      orchester_workspace::sharing::Connection::Connected | orchester_workspace::sharing::Connection::Checking
    )
  });

  open_sharing(&mut window, "sharing-make-local-button");
  press(&mut window, "sharing-make-local-button");
  shows(&mut window, "sharing-make-local-question");
  assert!(
    window
      .text("sharing-make-local-question-body")
      .contains("can't be reached")
  );
  assert!(
    window
      .text("sharing-make-local-tasks-value")
      .starts_with("All 2 stay as local tasks · as last synced at ")
  );
  assert_eq!(window.text("sharing-make-local-confirm"), "Make local anyway");
  press(&mut window, "sharing-make-local-confirm");
  shows(&mut window, "sharing-made-local");
  assert_eq!(local_keys(&window), ["KON-1", "KON-2"]);
}

#[test]
fn shared_project_make_local_says_which_not_shared_tasks_returned() {
  let (server, kontur) = server();
  let first = harness::window(Some(KONTUR_REMOTE));
  local_tasks(&first, &["Retry webhooks", "Show delivery status", "Paginate the log"]);
  join(&first, &server, &kontur, ("ivan", "Ivan Zaida"));
  link_kontur(&first);

  // Oleh's checkout joins with KON-1 and keeps KON-2 Not shared.
  let mut window = harness::window(Some(KONTUR_REMOTE));
  local_tasks(&window, &["Export invoices", "Retry exports"]);
  let recorder = join(&window, &server, &kontur, ("oleh", "Oleh Petrenko"));
  let services = window.launch.services().expect("services").clone();
  let root = kontur_root(&window);
  let review = block_on(
    &window,
    services.servers.join_review_at(&entry(&window), &root, "kontur"),
  )
  .expect("review");
  let ticked = vec![review.tasks[0].task.id()];
  block_on(
    &window,
    services
      .servers
      .join(&entry(&window), &root, &review, (ticked, IdempotencyKey::generate())),
  )
  .expect("joined");
  assert_eq!(server.task_keys(&kontur), ["KON-1", "KON-2", "KON-3", "KON-4"]);

  open_sharing(&mut window, "sharing-make-local-button");
  press(&mut window, "sharing-make-local-button");
  shows(&mut window, "sharing-make-local-not-shared");
  press(&mut window, "sharing-make-local-confirm");
  shows(&mut window, "sharing-made-local");
  assert_eq!(
    window.text("sharing-made-local-body"),
    "Its 4 tasks are local tasks now, with their numbers. The tasks kept Not shared returned: KON-2 → KON-5."
  );
  assert_eq!(local_keys(&window), ["KON-1", "KON-2", "KON-3", "KON-4", "KON-5"]);
  no_admin_endpoint(&recorder);
}
