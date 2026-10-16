//! N1 security review 1, B2 and M1: what changes who this computer trusts or what it sends is a person's. Each test
//! presses one such button with lurq's synthetic input, as an MCP client does, and finds the reason under it and that
//! nothing happened; the same press from a person goes through where a later step needs it.

use lurq::app::events::with_synthetic_input;
use orchester_api::client::{PlainHttp, ServerAddress};
use orchester_app::routes::{OrchesterView, ProjectPage, Route};
use orchester_workspace::sharing::{CodeRequest, Trust};

use super::{
  KONTUR_REMOTE,
  account_cloud::{AccountCloud, EMAIL, PASSWORD},
  clipboard::own_clipboard,
  harness::{block_on, go, press, shows, window},
  joined_through_recorder, link_kontur,
  server_support::TestServer,
  signed_in,
  support::{self, Window},
};

/// Presses `id` with synthetic input and finds why nothing happened under it.
fn refused(window: &mut Window, id: &str) {
  shows(window, id);
  with_synthetic_input(|| window.press(id));
  shows(window, &format!("{id}-person-only"));
}

#[test]
fn orchester_person_only_pairing_codes_and_their_copy() {
  let mut window = window(Some(KONTUR_REMOTE));
  let (_server, recorder) = joined_through_recorder(&window);
  let copied = own_clipboard(&mut window);
  let entry = window
    .launch
    .services()
    .expect("services")
    .servers
    .entries()
    .expect("entries")
    .remove(0);
  go(
    &mut window,
    Route::Orchester(OrchesterView::server(entry.stored.id)),
    "orchester-server-add-computer",
  );
  refused(&mut window, "orchester-server-add-computer");
  refused(&mut window, "orchester-server-admin-sign-in");
  assert!(window.tree.get_element_by_id("orchester-pairing-code").is_none());
  assert!(
    !recorder.lines().iter().any(|line| line.contains("/auth/pairings")),
    "no code was made: {:?}",
    recorder.lines()
  );
  press(&mut window, "orchester-server-add-computer");
  shows(&mut window, "orchester-pairing-copy");
  refused(&mut window, "orchester-pairing-copy");
  assert!(copied.lock().is_empty(), "nothing was copied");
  window.press("orchester-pairing-copy");
  assert_eq!(copied.lock().len(), 1, "a person copies it");
}

#[test]
fn orchester_person_only_cloud_address() {
  let mut window = window(None);
  go(
    &mut window,
    Route::Orchester(OrchesterView::default()),
    "orchester-advanced",
  );
  press(&mut window, "orchester-advanced");
  window.set("orchester-advanced-address", "http://localhost:9");
  refused(&mut window, "orchester-advanced-use");
  let services = window.launch.services().expect("services");
  assert!(
    services.cloud.address().expect("address").default,
    "the build's cloud stays"
  );
}

#[test]
fn orchester_person_only_cloud_sign_in_and_out() {
  let cloud = AccountCloud::start();
  let mut window = window(None);
  let services = window.launch.services().expect("services").clone();
  let address = ServerAddress::parse(&cloud.url()).expect("cloud address");
  block_on(&window, services.cloud.set_address(Some(address))).expect("a test cloud");
  go(
    &mut window,
    Route::Orchester(OrchesterView::default()),
    "orchester-sign-in-form",
  );
  window.set("orchester-sign-in-email", EMAIL);
  window.set("orchester-sign-in-password", PASSWORD);
  refused(&mut window, "orchester-sign-in");
  assert!(services.cloud.account().expect("account").is_none(), "not signed in");

  let mut window = signed_in(&cloud);
  refused(&mut window, "orchester-cloud-sign-out");
  let services = window.launch.services().expect("services");
  assert!(services.cloud.account().expect("account").is_some(), "still signed in");
  assert!(!cloud.signed_out());
}

#[test]
fn orchester_person_only_plain_http_to_another_computer() {
  let mut window = window(None);
  go(
    &mut window,
    Route::Orchester(OrchesterView::connect()),
    "orchester-connect-address",
  );
  window.set("orchester-connect-address", "http://orchester.example.test");
  refused(&mut window, "orchester-connect-allow-plain");
  let continue_button = window.texts("orchester-connect-continue").expect("continue");
  assert!(!continue_button.is_empty());
  window.press("orchester-connect-continue");
  window.pass();
  assert!(
    window.tree.get_element_by_id("orchester-connect-found").is_none(),
    "nothing was asked of the address without the allow"
  );
}

#[test]
fn orchester_person_only_joining_with_a_code() {
  let server = TestServer::start();
  server.create_user("alice", "Alice Moreau", true);
  let code = server.code_for("ivan", false);
  let mut window = window(None);
  go(
    &mut window,
    Route::Orchester(OrchesterView::connect()),
    "orchester-connect-address",
  );
  window.set("orchester-connect-address", &server.url());
  press(&mut window, "orchester-connect-continue");
  shows(&mut window, "orchester-connect-code");
  window.set("orchester-connect-code", code.expose());
  refused(&mut window, "orchester-connect-join");
  let services = window.launch.services().expect("services");
  assert!(
    services.servers.entries().expect("entries").is_empty(),
    "nothing joined"
  );
}

#[test]
fn orchester_person_only_accepting_and_declining_an_invite() {
  let cloud = AccountCloud::start();
  let server = TestServer::start();
  server.create_user("alice", "Alice Moreau", true);
  let audience = server.link_cloud(&cloud.root_key, &cloud.key_set);
  cloud.name_server(&audience, &server.url(), None);
  let invite = server.email_invite(EMAIL);
  cloud.invite(&invite, None);
  let mut window = signed_in(&cloud);
  let card = format!("orchester-invite-{invite}");
  refused(&mut window, &format!("{card}-accept"));
  assert_eq!(cloud.tokens_for(&invite), 0, "no token was asked for");
  let services = window.launch.services().expect("services").clone();
  assert!(
    block_on(&window, services.cloud.accepted_invites())
      .expect("kept")
      .is_empty()
  );
  press(&mut window, &format!("{card}-decline"));
  refused(&mut window, &format!("{card}-decline-confirm"));
  assert!(cloud.declined().is_empty(), "nothing was declined");
}

#[test]
fn orchester_person_only_linking_and_uploading() {
  let mut window = window(Some(KONTUR_REMOTE));
  let (_server, recorder) = joined_through_recorder(&window);
  go(
    &mut window,
    Route::Project(support::project_id(), ProjectPage::Settings),
    "settings-tab-sharing",
  );
  press(&mut window, "settings-tab-sharing");
  shows(&mut window, "sharing-address");
  window.set("sharing-address", &format!("{}/kontur", recorder.url()));
  let before = recorder.lines().len();
  refused(&mut window, "sharing-link-button");
  assert_eq!(recorder.lines().len(), before, "nothing was asked of the server");
  press(&mut window, "sharing-link-button");
  shows(&mut window, "sharing-link-confirm");
  refused(&mut window, "sharing-link-confirm");
  assert!(
    !recorder.lines().iter().any(|line| line.ends_with("/link")),
    "nothing was uploaded: {:?}",
    recorder.lines()
  );
}

#[test]
fn orchester_person_only_leaving_and_signing_out_of_a_server() {
  let mut window = window(Some(KONTUR_REMOTE));
  let (_server, recorder) = joined_through_recorder(&window);
  let entry = link_kontur(&window);
  go(
    &mut window,
    Route::Project(support::project_id(), ProjectPage::Settings),
    "settings-tab-sharing",
  );
  press(&mut window, "settings-tab-sharing");
  press(&mut window, "sharing-leave");
  refused(&mut window, "sharing-leave-confirm");
  assert!(
    !recorder.lines().iter().any(|line| line.starts_with("DELETE")),
    "nobody left: {:?}",
    recorder.lines()
  );

  go(
    &mut window,
    Route::Orchester(OrchesterView::server(entry.stored.id)),
    "orchester-server-sign-out",
  );
  press(&mut window, "orchester-server-sign-out");
  refused(&mut window, "orchester-server-sign-out-confirm");
  let services = window.launch.services().expect("services");
  assert!(
    services.servers.entries().expect("entries")[0].in_use(),
    "still signed in"
  );
}

#[test]
fn orchester_person_only_joining_again() {
  let server = TestServer::start();
  server.create_user("alice", "Alice Moreau", true);
  let code = server.code_for("ivan", false);
  let mut window = window(None);
  let services = window.launch.services().expect("services").clone();
  let address = ServerAddress::parse(&server.url()).expect("address");
  let request = CodeRequest {
    code,
    username: Some("ivan".into()),
    display_name: Some("Ivan Zaida".into()),
  };
  let joined = block_on(
    &window,
    services
      .servers
      .join_with_code(&address, Trust::System(PlainHttp::NotAllowed), request),
  )
  .expect("joined");
  let entry = services.servers.entries().expect("entries").remove(0);
  block_on(&window, services.servers.sign_out(&entry)).expect("signed out");
  go(
    &mut window,
    Route::Orchester(OrchesterView::server(joined)),
    "orchester-server-join-again",
  );
  refused(&mut window, "orchester-server-join-again");
}
