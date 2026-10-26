//! N3: a checkout with tasks linked to a project with tasks is joined through its review (`AiCwO`), with two people:
//! Alice made KTR on the server with tasks of her own, and this computer joins it with local tasks. The review sends
//! nothing until confirmed, ticks every task with the server's similar title as a hint, keeps a task whose run hasn't
//! ended ticked, uploads the ticked ones with new numbers and keeps the others *Not shared*; Sharing then says what the
//! join did and keeps the keys before joining. A join whose answer was lost keeps the ticks and, tried again, uploads
//! nothing twice. A *Not shared* task is shared later from its page, as a person's press only. An address of a project
//! the person isn't a member of says so and sends nothing more.

use lurq::app::events::with_synthetic_input;
use orchester_api::SharedProject;
use orchester_app::routes::{ProjectPage, Route, TaskList, TaskListTab, TaskTab, TaskView};
use orchester_core::{
  resource::ResourceKind,
  store::{ResourceStore, TaskStore},
  task::TaskDetails,
};
use orchester_store::ProjectDb;
use serde_json::json;

use super::{KONTUR_REMOTE, harness, join, kontur_root, local_tasks, no_admin_endpoint, open_sharing};
use crate::{
  harness::{Recorder, press, shows},
  server_support::TestServer,
  support::Window,
};

/// A team server with KTR (Kontur, set up for [`KONTUR_REMOTE`]) holding two of Alice's tasks, KTR-1 and KTR-2.
fn ktr() -> (TestServer, SharedProject) {
  let server = TestServer::start();
  server.create_user("alice", "Alice Moreau", true);
  let kontur = server.create_named_project(("Kontur", "KTR"), "kontur", Some(KONTUR_REMOTE));
  server.add_tasks(
    &kontur,
    &[
      json!({ "title": "Typed errors per module" }),
      json!({ "title": "Export invoices to PDF" }),
    ],
  );
  (server, kontur)
}

/// The window's Kontur with KON-1 … KON-3, joined to `server` as Oleh through a recording port; the review open.
fn reviewing(server: &TestServer, kontur: &SharedProject) -> (Window, Recorder) {
  let mut window = harness::window(Some(KONTUR_REMOTE));
  local_tasks(
    &window,
    &[
      "Export invoices as PDF",
      "Add retry to webhook sender",
      "Paginate the webhook log",
    ],
  );
  let recorder = join(&window, server, kontur, ("oleh", "Oleh Petrenko"));
  open_sharing(&mut window, "sharing-address");
  window.set("sharing-address", &format!("{}/kontur", recorder.url()));
  press(&mut window, "sharing-link-button");
  shows(&mut window, "sharing-join");
  (window, recorder)
}

#[test]
fn shared_project_join_review_uploads_the_ticked_tasks_and_keeps_the_mapping() {
  let (server, kontur) = ktr();
  let (mut window, recorder) = reviewing(&server, &kontur);
  assert_eq!(
    server.task_keys(&kontur),
    ["KTR-1", "KTR-2"],
    "nothing goes up before the confirm"
  );
  assert!(
    window
      .text("sharing-join-row-KON-1-similar")
      .contains("KTR-2 Export invoices to PDF"),
    "the server's similar title is a hint"
  );
  assert_eq!(window.text("sharing-join-confirm"), "Join and upload 3");
  press(&mut window, "sharing-join-row-KON-1-tick");
  window.until("KON-1 unticked", |window| {
    window.texts("sharing-join-row-KON-1-becomes") == Some(vec!["Not shared".to_owned()])
  });
  assert_eq!(window.text("sharing-join-row-KON-2-becomes"), "KTR-3");
  assert_eq!(window.text("sharing-join-confirm"), "Join and upload 2");

  with_synthetic_input(|| window.press("sharing-join-confirm"));
  shows(&mut window, "sharing-join-confirm-person-only");
  assert_eq!(
    server.task_keys(&kontur).len(),
    2,
    "an MCP client's press sends nothing"
  );

  press(&mut window, "sharing-join-confirm");
  shows(&mut window, "sharing-joined");
  assert_eq!(server.task_keys(&kontur), ["KTR-1", "KTR-2", "KTR-3", "KTR-4"]);
  assert_eq!(window.text("sharing-keys-KON-2-now"), "KTR-3");
  assert_eq!(window.text("sharing-keys-KON-1-now"), "Not shared");

  // The mapping stays once the join's note is dismissed, and after Sharing is read again.
  press(&mut window, "sharing-joined-dismiss");
  window.until("the note dismissed", |window| {
    window.tree.get_element_by_id("sharing-joined").is_none()
  });
  open_sharing(&mut window, "sharing-keys");
  assert_eq!(window.text("sharing-keys-KON-3-now"), "KTR-4");
  let db = ProjectDb::open(&kontur_root(&window)).expect("database");
  assert_eq!(db.not_shared_tasks().expect("kept").len(), 1);
  no_admin_endpoint(&recorder);
}

#[test]
fn shared_project_join_keeps_a_task_whose_run_has_not_ended_ticked() {
  let (server, kontur) = ktr();
  let (mut window, recorder) = reviewing_with_a_run(&server, &kontur);
  shows(&mut window, "sharing-join-row-KON-2-locked");
  press(&mut window, "sharing-join-row-KON-2-tick");
  window.pass();
  assert_eq!(
    window.text("sharing-join-row-KON-2-becomes"),
    "KTR-4",
    "a task whose run hasn't ended can't be unticked"
  );
  press(&mut window, "sharing-join-row-KON-1-tick");
  window.until("KON-1 unticked", |window| {
    window.texts("sharing-join-confirm") == Some(vec!["Join and upload 1".to_owned()])
  });
  press(&mut window, "sharing-join-confirm");
  shows(&mut window, "sharing-joined");
  assert_eq!(server.task_keys(&kontur), ["KTR-1", "KTR-2", "KTR-3"]);
  assert!(window.text("sharing-keys-KON-2-note").contains("claimed for you"));
  no_admin_endpoint(&recorder);
}

/// [`reviewing`] for a window whose Kontur has KON-1 and KON-2, whose run works.
fn reviewing_with_a_run(server: &TestServer, kontur: &SharedProject) -> (Window, Recorder) {
  let mut window = harness::window(Some(KONTUR_REMOTE));
  local_tasks(&window, &["Export invoices as PDF"]);
  working(&kontur_root(&window));
  let recorder = join(&window, server, kontur, ("oleh", "Oleh Petrenko"));
  open_sharing(&mut window, "sharing-address");
  window.set("sharing-address", &format!("{}/kontur", recorder.url()));
  press(&mut window, "sharing-link-button");
  shows(&mut window, "sharing-join");
  (window, recorder)
}

/// KON-2 of the checkout at `root`, complete on the project's published pipeline, with a run under way: a task with a
/// run is shared complete.
fn working(root: &std::path::Path) {
  let mut db = ProjectDb::open(root).expect("database");
  crate::run_fixture::auto_run(&mut db);
  let pipeline = db.resources(ResourceKind::Pipeline).expect("pipelines").remove(0).id();
  let task = db.tasks().expect("tasks").remove(1);
  let complete = TaskDetails {
    brief: "Review what the regression changed.".into(),
    acceptance_criteria: vec!["The reviewer's notes are answered.".into()],
    pipeline: Some(pipeline),
    ..task.details().clone()
  };
  db.update_task(task.id(), task.revision(), complete)
    .expect("complete the task")
    .saved()
    .expect("saved as read");
}

#[test]
fn shared_project_a_lost_join_answer_keeps_the_ticks_and_uploads_nothing_twice() {
  let (server, kontur) = ktr();
  let (mut window, recorder) = reviewing(&server, &kontur);
  press(&mut window, "sharing-join-row-KON-1-tick");
  window.until("KON-1 unticked", |window| {
    window.texts("sharing-join-confirm") == Some(vec!["Join and upload 2".to_owned()])
  });
  recorder.lose_answer_once(&format!("POST /api/v1/projects/{}/tasks/import", kontur.id));
  press(&mut window, "sharing-join-confirm");
  shows(&mut window, "sharing-join-failed");
  assert!(
    window
      .text("sharing-join-failed")
      .contains("trying again doesn't upload them twice"),
    "the join says some may have gone up"
  );
  assert_eq!(server.task_keys(&kontur).len(), 4, "the import reached the server");
  assert_eq!(
    window.text("sharing-join-row-KON-1-becomes"),
    "Not shared",
    "the ticks are kept"
  );
  assert_eq!(window.text("sharing-join-confirm"), "Try again");

  press(&mut window, "sharing-join-confirm");
  shows(&mut window, "sharing-joined");
  assert_eq!(
    server.task_keys(&kontur),
    ["KTR-1", "KTR-2", "KTR-3", "KTR-4"],
    "nothing went up twice"
  );
  no_admin_endpoint(&recorder);
}

#[test]
fn shared_project_a_not_shared_task_is_shared_later_from_its_page() {
  let (server, kontur) = ktr();
  let (mut window, recorder) = reviewing(&server, &kontur);
  press(&mut window, "sharing-join-row-KON-1-tick");
  window.until("KON-1 unticked", |window| {
    window.texts("sharing-join-confirm") == Some(vec!["Join and upload 2".to_owned()])
  });
  press(&mut window, "sharing-join-confirm");
  shows(&mut window, "sharing-joined");

  // The list: the Not shared filter, former keys and the search by them.
  let project = crate::support::project_id();
  let all = || TaskList::new(TaskListTab::All);
  harness::go(
    &mut window,
    Route::Project(project, ProjectPage::Tasks(all())),
    "tasks-not-shared",
  );
  assert_eq!(window.text("tasks-not-shared"), "Not shared · 1");
  assert_eq!(window.text("task-former-key-KTR-3"), "was KON-2");
  harness::go(
    &mut window,
    Route::Project(project, ProjectPage::Tasks(all().with_search("KON-2"))),
    "task-row-KTR-3",
  );
  assert_eq!(
    window.text("task-former-key-KTR-3"),
    "was KON-2",
    "found by its key before joining, which its row says"
  );
  harness::go(
    &mut window,
    Route::Project(project, ProjectPage::Tasks(all().with_search("KON-1"))),
    "tasks-only-not-shared",
  );
  press(&mut window, "tasks-show-not-shared");
  shows(&mut window, "task-row-KON-1");
  shows(&mut window, "tasks-not-shared-notice");

  // Its page: read-only, Share task asks in place, and only a person shares it.
  harness::go(
    &mut window,
    Route::Project(
      project,
      ProjectPage::Task("KON-1".parse().expect("key"), TaskView::of(TaskTab::Summary)),
    ),
    "task-not-shared",
  );
  press(&mut window, "task-share");
  shows(&mut window, "task-share-question");
  window.until("the next key", |window| {
    window.texts("task-share-confirm") == Some(vec!["Share as KTR-5".to_owned()])
  });
  with_synthetic_input(|| window.press("task-share-confirm"));
  shows(&mut window, "task-share-confirm-person-only");
  assert_eq!(
    server.task_keys(&kontur).len(),
    4,
    "an MCP client's press shares nothing"
  );
  press(&mut window, "task-share-confirm");
  shows(&mut window, "task-former-key");
  assert_eq!(window.text("task-former-key"), "was KON-1");
  assert_eq!(server.task_keys(&kontur), ["KTR-1", "KTR-2", "KTR-3", "KTR-4", "KTR-5"]);
  no_admin_endpoint(&recorder);
}

#[test]
fn shared_project_an_address_without_membership_sends_nothing_more() {
  let (server, kontur) = ktr();
  let billing = server.create_named_project(("Billing", "BIL"), "billing", None);
  let mut window = harness::window(Some(KONTUR_REMOTE));
  local_tasks(&window, &["Export invoices as PDF"]);
  let recorder = join(&window, &server, &kontur, ("oleh", "Oleh Petrenko"));
  open_sharing(&mut window, "sharing-address");
  window.set("sharing-address", &format!("{}/billing", recorder.url()));
  press(&mut window, "sharing-link-button");
  shows(&mut window, "sharing-no-project");
  assert!(server.task_keys(&billing).is_empty());
  let lines = recorder.lines();
  assert!(
    !lines.iter().any(|line| line.contains("/tasks")),
    "nothing about tasks was asked or sent: {lines:?}"
  );
  no_admin_endpoint(&recorder);
}
