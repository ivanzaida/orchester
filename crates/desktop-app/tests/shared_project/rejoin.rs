//! N3 review: a checkout links to one project. Once joined, the join's address for the same project shows its Sharing,
//! and one for another server's project reads and sends nothing (M1). A task kept *Not shared* has an address of its
//! own: with the same key on both sides, its page is its own and the server's task with that key is left alone (M2).

use orchester_app::routes::{JoinTarget, ProjectPage, Route, TaskList, TaskTab, TaskView};
use orchester_store::ProjectDb;
use serde_json::json;

use super::{KONTUR_REMOTE, entry, harness, join, kontur_root, local_tasks, no_admin_endpoint, open_sharing};
use crate::{
  harness::{press, shows},
  server_support::TestServer,
  support::{self, Window},
};

/// The window's Kontur (KON-1 … KON-3) joined to Kontur on `server` with KON-2 unticked, through the review.
fn joined(window: &mut Window, server: &TestServer, kontur: &orchester_api::SharedProject) -> super::harness::Recorder {
  let recorder = join(window, server, kontur, ("oleh", "Oleh Petrenko"));
  open_sharing(window, "sharing-address");
  window.set("sharing-address", &format!("{}/kontur", recorder.url()));
  press(window, "sharing-link-button");
  press(window, "sharing-join-row-KON-2-tick");
  window.until("KON-2 unticked", |window| {
    window.texts("sharing-join-confirm") == Some(vec!["Join and upload 2".to_owned()])
  });
  press(window, "sharing-join-confirm");
  shows(window, "sharing-joined");
  recorder
}

#[test]
fn shared_project_a_linked_checkout_never_joins_again_or_elsewhere() {
  let server = TestServer::start();
  server.create_user("alice", "Alice Moreau", true);
  let kontur = server.create_named_project(("Kontur", "KTR"), "kontur", Some(KONTUR_REMOTE));
  server.add_tasks(&kontur, &[json!({ "title": "Typed errors per module" })]);
  let mut window = harness::window(Some(KONTUR_REMOTE));
  local_tasks(
    &window,
    &["Export invoices as PDF", "Retry exports", "Paginate the log"],
  );
  let recorder = joined(&mut window, &server, &kontur);
  let keys = server.task_keys(&kontur);

  // The join's address for the project it joined: its Sharing, nothing joined again.
  let project = support::project_id();
  let target = JoinTarget::new(entry(&window).stored.id, "kontur").expect("target");
  harness::go(
    &mut window,
    Route::Project(project, ProjectPage::Join(target)),
    "sharing-keys",
  );
  assert_eq!(server.task_keys(&kontur), keys, "nothing went up again");

  // Another server's project: nothing is read or sent, and the checkout stays linked to Kontur.
  let other = TestServer::start();
  other.create_user("maria", "Maria Lind", true);
  let atlas = other.create_named_project(("Atlas", "ATL"), "atlas", Some(KONTUR_REMOTE));
  let elsewhere = join(&window, &other, &atlas, ("oleh", "Oleh Petrenko"));
  let services = window.launch.services().expect("services").clone();
  let second = services
    .servers
    .entries()
    .expect("entries")
    .into_iter()
    .find(|entry| entry.stored.id != super::entry(&window).stored.id)
    .expect("the second server");
  let target = JoinTarget::new(second.stored.id, "atlas").expect("target");
  harness::go(
    &mut window,
    Route::Project(project, ProjectPage::Join(target)),
    "sharing-join-unavailable",
  );
  assert!(other.task_keys(&atlas).is_empty(), "nothing went up into Atlas");
  assert!(
    !elsewhere.lines().iter().any(|line| line.contains("/tasks")),
    "nothing about tasks was asked there: {:?}",
    elsewhere.lines()
  );
  let link = ProjectDb::open(&kontur_root(&window))
    .expect("database")
    .sharing()
    .expect("link")
    .expect("still linked");
  assert_eq!(link.project, kontur.id);
  no_admin_endpoint(&recorder);
}

#[test]
fn shared_project_a_not_shared_task_with_a_shared_key_has_its_own_page() {
  // The server project's key is the checkout's own: KON-2 is Alice's there and Ivan's Not shared task here.
  let server = TestServer::start();
  server.create_user("alice", "Alice Moreau", true);
  let kontur = server.create_named_project(("Kontur", "KON"), "kontur", Some(KONTUR_REMOTE));
  server.add_tasks(
    &kontur,
    &[
      json!({ "title": "Typed errors per module" }),
      json!({ "title": "Alice's own KON-2" }),
    ],
  );
  let mut window = harness::window(Some(KONTUR_REMOTE));
  local_tasks(
    &window,
    &["Export invoices as PDF", "Retry exports", "Paginate the log"],
  );
  let recorder = joined(&mut window, &server, &kontur);
  let project = support::project_id();

  // From the list's Not shared filter, the row opens the kept task's own page.
  harness::go(
    &mut window,
    Route::Project(project, ProjectPage::Tasks(TaskList::default().with_not_shared(true))),
    "task-row-KON-2",
  );
  press(&mut window, "task-row-KON-2");
  shows(&mut window, "task-not-shared");
  assert_eq!(window.text("task-title"), "Retry exports");

  // The shared KON-2 is Alice's, on its own address.
  harness::go(
    &mut window,
    Route::Project(
      project,
      ProjectPage::Task("KON-2".parse().expect("key"), TaskView::of(TaskTab::Summary)),
    ),
    "task-title",
  );
  window.until("Alice's task", |window| {
    window.texts("task-title") == Some(vec!["Alice's own KON-2".to_owned()])
  });
  assert!(window.tree.get_element_by_id("task-not-shared").is_none());

  // Share task from the kept page leaves Alice's KON-2 as it was.
  harness::go(
    &mut window,
    Route::Project(
      project,
      ProjectPage::KeptTask("KON-2".parse().expect("key"), TaskView::of(TaskTab::Summary)),
    ),
    "task-share",
  );
  press(&mut window, "task-share");
  window.until("the next key", |window| {
    window.texts("task-share-confirm") == Some(vec!["Share as KON-5".to_owned()])
  });
  press(&mut window, "task-share-confirm");
  shows(&mut window, "task-former-key");
  assert_eq!(window.text("task-title"), "Retry exports");
  assert_eq!(server.task_keys(&kontur), ["KON-1", "KON-2", "KON-3", "KON-4", "KON-5"]);
  let db = ProjectDb::open(&kontur_root(&window)).expect("database");
  let alices = orchester_core::store::TaskStore::tasks(&db)
    .expect("tasks")
    .into_iter()
    .find(|task| task.key().to_string() == "KON-2")
    .expect("KON-2");
  assert_eq!(
    alices.details().title.as_str(),
    "Alice's own KON-2",
    "the server's KON-2 is untouched"
  );
  no_admin_endpoint(&recorder);
}
