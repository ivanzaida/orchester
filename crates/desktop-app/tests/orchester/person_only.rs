//! N1 security review 1, B2 and M1: what changes who this computer trusts or what it sends is a person's. Each test
//! presses one such button with lurq's synthetic input, as an MCP client does, and finds the reason under it and that
//! nothing happened; the same press from a person goes through where a later step needs it. The reason is the
//! Orchestrator's: the hand in `warning` (N1 design 2).

use lurq::app::events::with_synthetic_input;
use orchester_api::client::{Client, PlainHttp, ServerAddress};
use orchester_app::{
  routes::{OrchesterView, ProjectPage, Route},
  ui::icon::{Icon, glyph},
};
use orchester_workspace::sharing::{CodeRequest, Trust, trust::PresentedCertificate};

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
  let reason = format!("{id}-person-only");
  shows(window, &reason);
  in_warning(window, &reason);
}

/// The reason `id` is the Orchestrator's person-only note (`ui::tinted_note::person_only`): the hand beside its words,
/// not the danger field error. The tree does not carry a text's color; the component gives both `warning`.
fn in_warning(window: &mut Window, id: &str) {
  fn leaves(element: lurq::node::element::ElementRef<'_>, into: &mut Vec<String>) {
    into.extend(element.text_content().map(str::to_owned));
    for child in element.children() {
      leaves(child, into);
    }
  }
  let mut parts = Vec::new();
  leaves(window.tree.get_element_by_id(id).expect("the reason"), &mut parts);
  assert_eq!(
    parts,
    [
      glyph(Icon::Hand),
      "Only a person does this here: input from an MCP client is refused. Clients can still read this page.".to_owned()
    ],
    "the hand beside the words"
  );
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
  let before = window
    .launch
    .services()
    .expect("services")
    .cloud
    .address()
    .expect("address");
  go(
    &mut window,
    Route::Orchester(OrchesterView::default()),
    "orchester-advanced",
  );
  press(&mut window, "orchester-advanced");
  window.set("orchester-advanced-address", "http://localhost:9");
  refused(&mut window, "orchester-advanced-use");
  let services = window.launch.services().expect("services");
  assert_eq!(
    services.cloud.address().expect("address"),
    before,
    "the cloud in use stays"
  );
  assert!(services.cloud.is_signed_in(), "and the sign-in with it");
}

#[test]
fn orchester_person_only_cloud_sign_in_and_out() {
  let cloud = AccountCloud::start();
  let mut window = window(None);
  let services = window.launch.services().expect("services").clone();
  let address = ServerAddress::parse(&cloud.url()).expect("cloud address");
  // Another cloud signs out: the sign-in is the gate's.
  block_on(&window, services.cloud.set_address(Some(address))).expect("a test cloud");
  shows(&mut window, "gate-email");
  window.set("gate-email", EMAIL);
  window.set("gate-password", PASSWORD);
  refused(&mut window, "gate-sign-in");
  assert!(services.cloud.account().expect("account").is_none(), "not signed in");

  let mut window = signed_in(&cloud);
  // *Sign out* asks; signing out and locking is a person's.
  press(&mut window, "orchester-cloud-sign-out");
  refused(&mut window, "orchester-cloud-sign-out-confirm");
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
    Route::Project(support::project_id(), ProjectPage::Settings(Default::default())),
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
    Route::Project(support::project_id(), ProjectPage::Settings(Default::default())),
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
  let joined = block_on(
    &window,
    services
      .servers
      .join_with_code(&address, Trust::System(PlainHttp::NotAllowed), as_ivan(&server, code)),
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

/// A code request for `ivan` with `code`, from a computer signed in to the cloud with two-factor authentication.
fn as_ivan(server: &TestServer, code: orchester_api::Credential) -> CodeRequest {
  CodeRequest {
    code,
    username: Some("ivan".into()),
    display_name: Some("Ivan Zaida".into()),
    proof: Some(server.code_proof("ivan")),
  }
}

#[test]
fn orchester_person_only_check_code_when_connecting() {
  let mut server = TestServer::start();
  server.create_user("alice", "Alice Moreau", true);
  server.serve_self_signed();
  let mut window = window(None);
  go(
    &mut window,
    Route::Orchester(OrchesterView::connect()),
    "orchester-connect-address",
  );
  window.set("orchester-connect-address", &server.url());
  press(&mut window, "orchester-connect-continue");
  refused(&mut window, "orchester-connect-matches");
  window.pass();
  assert!(
    window.tree.get_element_by_id("orchester-connect-code").is_none(),
    "the check code was not confirmed"
  );
  press(&mut window, "orchester-connect-matches");
  shows(&mut window, "orchester-connect-code");
}

#[test]
fn orchester_person_only_check_code_of_a_changed_certificate() {
  let mut server = TestServer::start();
  server.create_user("alice", "Alice Moreau", true);
  let code = server.code_for("ivan", false);
  server.serve_self_signed();
  let mut other = TestServer::start();
  other.serve_self_signed();
  let mut window = window(None);
  let services = window.launch.services().expect("services").clone();
  let address = ServerAddress::parse(&server.url()).expect("address");
  let presented = block_on(&window, PresentedCertificate::read(&address)).expect("certificate");
  let joined = block_on(
    &window,
    services
      .servers
      .join_with_code(&address, presented.confirm(), as_ivan(&server, code)),
  )
  .expect("joined");
  // This computer now trusts another server's certificate, so this one's looks changed.
  let elsewhere = ServerAddress::parse(&other.url()).expect("address");
  let stale = other
    .block_on(Client::presented_certificate(&elsewhere))
    .expect("certificate");
  let wrong = block_on(&window, PresentedCertificate::read(&elsewhere)).expect("certificate");
  let entry = services.servers.entries().expect("entries").remove(0);
  block_on(&window, services.servers.trust_again(&entry, wrong.confirm())).expect("trusted");
  go(
    &mut window,
    Route::Orchester(OrchesterView::server(joined)),
    "orchester-server-certificate-matches",
  );
  refused(&mut window, "orchester-server-certificate-matches");
  window.pass();
  let entry = services.servers.entries().expect("entries").remove(0);
  assert_eq!(
    entry.stored.certificate_pin,
    Some(stale.to_hex()),
    "the new certificate was not trusted"
  );
}
