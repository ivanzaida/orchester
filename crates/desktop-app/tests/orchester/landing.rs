//! N3: a project invite accepted lands in the join review (`xeFRB`, `d8oDsO`): with the one checkout whose `origin`
//! matches the project's hint it opens at once; otherwise the landing lists the checkouts to pick, and *Continue to the
//! join review* opens it in the one picked. Nothing is uploaded before the review is confirmed.

use orchester_api::SharedProject;
use orchester_app::routes::{JoinTarget, ProjectPage, Route};
use orchester_core::{
  id::IdempotencyKey,
  name::Name,
  store::TaskStore,
  task::{TaskDetails, TaskLimits},
  time::Timestamp,
};
use orchester_store::ProjectDb;
use serde_json::json;

use super::{
  KONTUR_REMOTE,
  harness::{press, shows},
  server_support::TestServer,
  signed_in_with,
  support::{
    self, Window,
    signed_in::account_cloud::{AccountCloud, EMAIL},
  },
};

/// A plain team server linked to `cloud`, with Kontur (KTR) holding one of Alice's tasks, and an invite to it for the
/// account; the invite's id.
fn invited(cloud: &AccountCloud) -> (TestServer, SharedProject, String) {
  let server = TestServer::start();
  server.create_user("alice", "Alice Moreau", true);
  let audience = server.link_cloud(&cloud.root_key, &cloud.key_set);
  cloud.name_server(&audience, &server.url(), None);
  let kontur = server.create_named_project(("Kontur", "KTR"), "kontur", Some(KONTUR_REMOTE));
  server.add_tasks(&kontur, &[json!({ "title": "Typed errors per module" })]);
  let invite = server.project_email_invite(EMAIL, &kontur);
  cloud.invite(&invite, Some(("Kontur", &format!("{}/kontur", server.url()))));
  (server, kontur, invite)
}

/// A local task titled `title` in the window's Kontur.
fn local_task(window: &Window, title: &str) {
  let details = TaskDetails {
    title: Name::new(title).expect("a title"),
    brief: String::new(),
    acceptance_criteria: Vec::new(),
    pipeline: None,
    limits: TaskLimits::NONE,
  };
  ProjectDb::open(&window.data.repo("repo-kontur"))
    .expect("Kontur's database")
    .create_tasks(IdempotencyKey::generate(), vec![details], Timestamp::now())
    .expect("a local task");
}

#[test]
fn orchester_a_project_invite_lands_in_the_join_review() {
  let cloud = AccountCloud::start();
  let (server, kontur, invite) = invited(&cloud);
  let mut window = signed_in_with(&cloud, Some(KONTUR_REMOTE));
  local_task(&window, "Export invoices as PDF");
  press(&mut window, &format!("orchester-invite-{invite}-accept"));
  // The one checkout that matches the project's hint: the landing is skipped.
  shows(&mut window, "sharing-join");
  assert_eq!(
    server.task_keys(&kontur),
    ["KTR-1"],
    "nothing goes up before the review is confirmed"
  );
  press(&mut window, "sharing-join-confirm");
  shows(&mut window, "sharing-joined");
  assert_eq!(server.task_keys(&kontur), ["KTR-1", "KTR-2"]);
}

#[test]
fn orchester_the_invite_landing_lists_the_checkouts_to_pick() {
  let cloud = AccountCloud::start();
  let (server, kontur, invite) = invited(&cloud);
  let mut window = signed_in_with(&cloud, Some("https://github.com/oleh/kontur.git"));
  local_task(&window, "Export invoices as PDF");
  press(&mut window, &format!("orchester-invite-{invite}-accept"));
  // Kontur here is a fork of the project's repository: it can be picked, with the warning, so the landing asks.
  shows(&mut window, "orchester-landing");
  let row = format!("orchester-landing-checkout-{}", support::project_id());
  assert!(
    window
      .text(&format!("{row}-origin"))
      .contains("differs from github.com/acme/kontur")
  );
  press(&mut window, &format!("{row}-pick"));
  press(&mut window, "orchester-landing-continue");
  shows(&mut window, "sharing-join-mismatch");
  let services = window.launch.services().expect("services").clone();
  let server_id = services.servers.entries().expect("entries")[0].stored.id;
  let target = JoinTarget::new(server_id, "kontur").expect("target");
  assert_eq!(
    window.navigation().route_now(),
    Route::Project(support::project_id(), ProjectPage::Join(target))
  );
  assert_eq!(server.task_keys(&kontur), ["KTR-1"], "nothing was uploaded");
}

#[test]
fn orchester_an_invite_to_a_project_linked_here_opens_its_sharing() {
  let cloud = AccountCloud::start();
  let (server, kontur, invite) = invited(&cloud);
  let mut window = signed_in_with(&cloud, Some(KONTUR_REMOTE));
  local_task(&window, "Export invoices as PDF");
  press(&mut window, &format!("orchester-invite-{invite}-accept"));
  press(&mut window, "sharing-join-confirm");
  shows(&mut window, "sharing-joined");
  let keys = server.task_keys(&kontur);

  // Invited again, to the project this checkout is linked to: accepting it shows the project's Sharing.
  let again = server.project_email_invite(EMAIL, &kontur);
  cloud.invite(&again, Some(("Kontur", &format!("{}/kontur", server.url()))));
  super::harness::go(
    &mut window,
    Route::Orchester(orchester_app::routes::OrchesterView::default()),
    &format!("orchester-invite-{again}-accept"),
  );
  press(&mut window, &format!("orchester-invite-{again}-accept"));
  shows(&mut window, "sharing-keys");
  assert!(window.tree.get_element_by_id("sharing-join").is_none(), "no review");
  assert_eq!(server.task_keys(&kontur), keys, "nothing went up again");
}
