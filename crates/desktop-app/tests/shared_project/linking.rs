//! The first checkout linked to an empty project uploads its tasks with their numbers on the first link's page
//! (`KSnOW`), and writes `orchester.toml` when the person leaves the switch on; a later checkout without tasks takes
//! the project's tasks as they are, and one with tasks gets new numbers (its review is N3's, so the window says so and
//! sends nothing; the join runs through the workspace as that review will).

use std::fs;

use orchester_core::id::IdempotencyKey;
use orchester_store::ProjectDb;

use super::{
  KONTUR_REMOTE, entry, harness, join, kontur_root, local_keys, local_tasks, no_admin_endpoint, open_sharing, server,
};
use crate::harness::{block_on, press, shows};

#[test]
fn shared_project_first_link_uploads_the_tasks_with_their_numbers() {
  let (server, kontur) = server();
  let mut window = harness::window(Some(KONTUR_REMOTE));
  local_tasks(&window, &["Retry webhooks", "Show delivery status", "Paginate the log"]);
  let recorder = join(&window, &server, &kontur, ("ivan", "Ivan Zaida"));
  open_sharing(&mut window, "sharing-address");
  window.set("sharing-address", &format!("{}/kontur", recorder.url()));
  press(&mut window, "sharing-link-button");
  shows(&mut window, "sharing-upload");
  assert!(
    server.task_keys(&kontur).is_empty(),
    "nothing goes up before the person confirms"
  );
  assert_eq!(
    window.text("sharing-upload-tasks-value"),
    "3 · their numbers are kept, KON-1 … KON-3"
  );
  assert_eq!(
    window.text("sharing-upload-next-value"),
    "Continue at KON-4, numbered by the server"
  );
  assert!(window.text("sharing-upload-title").starts_with("Link Kontur to "));
  press(&mut window, "sharing-link-confirm");
  shows(&mut window, "sharing-shared");

  assert_eq!(server.task_keys(&kontur), ["KON-1", "KON-2", "KON-3"]);
  assert_eq!(local_keys(&window), ["KON-1", "KON-2", "KON-3"]);
  let written = fs::read_to_string(kontur_root(&window).join("orchester.toml")).expect("orchester.toml");
  assert_eq!(written, format!("project = \"{}/kontur\"\n", recorder.url()));
  shows(&mut window, "sharing-discovery-file");
  no_admin_endpoint(&recorder);
}

#[test]
fn shared_project_later_checkouts_get_the_tasks_or_new_numbers() {
  let (server, kontur) = server();
  let first = harness::window(Some(KONTUR_REMOTE));
  local_tasks(&first, &["Retry webhooks", "Show delivery status", "Paginate the log"]);
  join(&first, &server, &kontur, ("ivan", "Ivan Zaida"));
  super::link_kontur(&first);

  // A checkout without tasks takes the project's as they are.
  let mut empty = harness::window(Some(KONTUR_REMOTE));
  let recorder = join(&empty, &server, &kontur, ("maria", "Maria Lind"));
  open_sharing(&mut empty, "sharing-address");
  empty.set("sharing-address", &format!("{}/kontur", recorder.url()));
  press(&mut empty, "sharing-link-button");
  shows(&mut empty, "sharing-upload");
  assert_eq!(empty.text("sharing-upload-what-title"), "What comes down");
  assert_eq!(empty.text("sharing-link-confirm"), "Link");
  press(&mut empty, "sharing-link-confirm");
  shows(&mut empty, "sharing-shared");
  assert_eq!(local_keys(&empty), ["KON-1", "KON-2", "KON-3"]);
  no_admin_endpoint(&recorder);

  // A checkout with tasks joins with new numbers after its review, which is N3's: the window sends nothing.
  let mut other = harness::window(Some(KONTUR_REMOTE));
  local_tasks(&other, &["Export invoices", "Retry exports"]);
  let recorder = join(&other, &server, &kontur, ("oleh", "Oleh Petrenko"));
  open_sharing(&mut other, "sharing-address");
  other.set("sharing-address", &format!("{}/kontur", recorder.url()));
  press(&mut other, "sharing-link-button");
  shows(&mut other, "sharing-taken");
  assert_eq!(server.task_keys(&kontur).len(), 3, "nothing was uploaded");

  let services = other.launch.services().expect("services").clone();
  let root = kontur_root(&other);
  let review = block_on(&other, services.servers.join_review_at(&entry(&other), &root, "kontur")).expect("review");
  let ticked = review.tasks.iter().map(|task| task.task.id()).collect();
  block_on(
    &other,
    services
      .servers
      .join(&entry(&other), &root, &review, (ticked, IdempotencyKey::generate())),
  )
  .expect("joined");
  assert_eq!(server.task_keys(&kontur), ["KON-1", "KON-2", "KON-3", "KON-4", "KON-5"]);
  let former = ProjectDb::open(&root)
    .expect("database")
    .former_keys()
    .expect("former keys");
  let mut kept: Vec<&String> = former.values().collect();
  kept.sort();
  assert_eq!(kept, ["KON-1", "KON-2"], "the tasks keep their keys before joining");
  no_admin_endpoint(&recorder);
}
