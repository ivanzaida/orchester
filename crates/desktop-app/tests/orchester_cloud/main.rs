//! N1 in the window against Orchester Cloud and a team server built from this tree, on loopback: the cloud on a
//! PostgreSQL of its own (`real_cloud`), the team server presenting its own certificate and linked to the cloud as
//! its admin links it. An invite by email reaches the app through the cloud and is accepted there, which joins the
//! server at the address and with the certificate the cloud's registry has for it; another is declined; a person who
//! signed out joins again with the account. Linking, codes, signing out with projects and the check that the app
//! never calls an admin endpoint run against the same team server in `tests/orchester`.
//!
//! The tests need a PostgreSQL whose user may create databases, named by `ORCHESTER_CLOUD_TEST_DATABASE_URL`
//! (`postgres://user:password@host:port`); without it they are ignored, never passed:
//! `cargo test -p orchester-app --test orchester_cloud orchester_cloud -- --ignored`, after
//! `cargo build -p orchester-cloud`.

#![allow(
  clippy::print_stderr,
  reason = "the tests say on the console what they could not clean up: a database or the cloud's process"
)]

#[path = "../orchester/harness.rs"]
#[allow(dead_code, reason = "the recording port is for tests/orchester's plain-HTTP server")]
mod harness;
mod real_cloud;
#[path = "../../../workspace/tests/server_support/mod.rs"]
mod server_support;
#[path = "../support/mod.rs"]
mod support;

use harness::{block_on, go, press, shows, window};
use orchester_api::client::{Client, ServerAddress};
use orchester_app::routes::{OrchesterView, Route};
use orchester_store::JoinedWith;
use real_cloud::RealCloud;
use server_support::TestServer;

const ALICE: &str = "alice@example.test";
const BOHDAN: &str = "bohdan@example.test";
const PASSWORD: &str = "correct horse battery staple";

/// The cloud, and a team server linked to it by Alice, its admin, with the project Billing.
fn linked() -> (RealCloud, TestServer, orchester_api::SharedProject, String) {
  let cloud = RealCloud::start();
  let server = TestServer::start_tls_linkable();
  server.create_user("alice", "Alice Moreau", true);
  let billing = server.create_named_project(
    ("Billing", "BIL"),
    "billing",
    Some("https://github.com/acme/billing.git"),
  );
  cloud.account(ALICE, PASSWORD, "Alice Moreau");
  let alice = cloud.device(ALICE, PASSWORD);
  let registered = cloud.link(&alice, &server.cloud_statement());
  server.finish_cloud_link(
    &registered.root_key,
    &registered.key_set,
    registered.identity_token.expose(),
  );
  cloud.account(BOHDAN, PASSWORD, "Bohdan Koval");
  let id = registered.server.server.id.to_string();
  (cloud, server, billing, id)
}

/// The window signed in to `cloud` as Bohdan, at Settings › Orchester.
fn signed_in(cloud: &RealCloud) -> support::Window {
  let mut window = window(None);
  let services = window.launch.services().expect("services").clone();
  let address = ServerAddress::parse(&cloud.url()).expect("cloud address");
  block_on(&window, services.cloud.set_address(Some(address))).expect("the test cloud");
  go(
    &mut window,
    Route::Orchester(OrchesterView::default()),
    "orchester-sign-in-form",
  );
  window.set("orchester-sign-in-email", BOHDAN);
  window.set("orchester-sign-in-password", PASSWORD);
  press(&mut window, "orchester-sign-in");
  shows(&mut window, "orchester-cloud-details");
  window
}

#[test]
#[ignore = "needs ORCHESTER_CLOUD_TEST_DATABASE_URL; run with --ignored"]
fn orchester_cloud_accepts_an_invite_and_joins_again_with_the_account() {
  let (cloud, server, billing, server_id) = linked();
  let (invite, signed) = server.signed_email_invite(BOHDAN, Some(&billing));
  cloud.deliver(&signed);
  let mut window = signed_in(&cloud);
  let services = window.launch.services().expect("services").clone();
  let accept = format!("orchester-invite-{invite}-accept");
  shows(&mut window, &accept);
  press(&mut window, &accept);
  shows(&mut window, "orchester-invite-joined");

  let entry = services.servers.entries().expect("entries").remove(0);
  let presented = server
    .block_on(Client::presented_certificate(
      &ServerAddress::parse(&server.url()).expect("address"),
    ))
    .expect("certificate");
  assert_eq!(
    entry.stored.certificate_pin,
    Some(presented.to_hex()),
    "pinned from the registry"
  );
  assert_eq!(
    entry.stored.joined_with,
    Some(JoinedWith::Cloud(server_id.parse().expect("server id")))
  );
  let projects = block_on(&window, services.servers.server_projects(&entry)).expect("the server let the device in");
  assert!(projects.iter().any(|project| project.project.slug == "billing"));
  assert!(
    block_on(&window, services.cloud.invites()).expect("invites").is_empty(),
    "the cloud's copy ended"
  );

  go(
    &mut window,
    Route::Orchester(OrchesterView::server(entry.stored.id)),
    "orchester-server-sign-out",
  );
  press(&mut window, "orchester-server-sign-out");
  press(&mut window, "orchester-server-sign-out-confirm");
  shows(&mut window, "orchester-server-join-again");
  press(&mut window, "orchester-server-join-again");
  window.until("joined again", |window| {
    for refusal in ["orchester-server-join-error", "orchester-server-join-not-invited"] {
      if let Some(texts) = window.texts(refusal) {
        panic!("joining again was refused: {}", texts.join(" "));
      }
    }
    services
      .servers
      .entries()
      .is_ok_and(|entries| entries.iter().any(orchester_workspace::sharing::ServerEntry::in_use))
  });
  let entries = services.servers.entries().expect("entries");
  let summary: Vec<_> = entries
    .iter()
    .map(|entry| (entry.stored.id, entry.address.as_str().to_owned(), entry.in_use()))
    .collect();
  assert_eq!(entries.len(), 1, "the same server: {summary:?}");
  assert!(entries[0].in_use(), "joined again with the account: {summary:?}");
}

#[test]
#[ignore = "needs ORCHESTER_CLOUD_TEST_DATABASE_URL; run with --ignored"]
fn orchester_cloud_declines_an_invite_in_place() {
  let (cloud, server, _billing, _server_id) = linked();
  let (invite, signed) = server.signed_email_invite(BOHDAN, None);
  cloud.deliver(&signed);
  let mut window = signed_in(&cloud);
  let services = window.launch.services().expect("services").clone();
  let decline = format!("orchester-invite-{invite}-decline");
  shows(&mut window, &decline);
  press(&mut window, &decline);
  press(&mut window, &format!("orchester-invite-{invite}-decline-confirm"));
  window.until("the invite gone", |window| {
    window
      .tree
      .get_element_by_id(&format!("orchester-invite-{invite}"))
      .is_none()
  });
  assert!(block_on(&window, services.cloud.invites()).expect("invites").is_empty());
  assert!(
    services.servers.entries().expect("entries").is_empty(),
    "nothing was joined"
  );
}
