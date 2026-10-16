//! Settings › Orchester, *Connect a server* and Project settings › Sharing (N1) in the window, against a real team
//! server and a stand-in for Orchester Cloud's account API on loopback: signing in, the cloud's pages opened in the
//! browser, accepting an invite with the certificate pin it carries, connecting with an invite code after the check
//! code, linking a local project by address with and without a remote mismatch, a pairing code for a second computer,
//! signing out naming the projects it affects, and that the app never calls an endpoint of the server admin. The
//! sign-in against the real cloud is in `crates/cloud/tests/cloud/desktop.rs`.
//!
//! `cargo test -p orchester-app --test orchester orchester_`

#[path = "../../../workspace/tests/sharing_support/account_cloud.rs"]
mod account_cloud;
mod harness;
#[path = "../../../workspace/tests/server_support/mod.rs"]
mod server_support;
#[path = "../support/mod.rs"]
mod support;

use account_cloud::{AccountCloud, EMAIL, PASSWORD};
use harness::{Recorder, administrative, block_on, go, press, shows, window};
use orchester_api::{
  CodeJoin, Credential,
  client::{Client, PlainHttp, ServerAddress},
};
use orchester_app::routes::{OrchesterView, ProjectPage, Route};
use orchester_store::JoinedWith;
use orchester_workspace::sharing::{CodeRequest, Trust, cloud::BUILD_ADDRESS};
use server_support::TestServer;

const KONTUR_REMOTE: &str = "https://github.com/acme/kontur.git";
const BILLING_REMOTE: &str = "https://github.com/acme/billing.git";

#[test]
fn orchester_web_links_open_the_clouds_pages() {
  let mut window = window(None);
  go(
    &mut window,
    Route::Orchester(OrchesterView::default()),
    "orchester-sign-in-form",
  );
  press(&mut window, "orchester-create-account");
  press(&mut window, "orchester-forgot");
  shows(&mut window, "orchester-opened");
  let links = window.launch.services().expect("services").links.recorded();
  assert_eq!(
    links,
    vec![format!("{BUILD_ADDRESS}/signup"), format!("{BUILD_ADDRESS}/forgot")]
  );
}

#[test]
fn orchester_signs_in_and_accepts_an_invite_with_its_pin() {
  let cloud = AccountCloud::start(true);
  let mut server = TestServer::start();
  server.create_user("alice", "Alice Moreau", true);
  let audience = server.link_cloud(&cloud.root_key, &cloud.key_set);
  let billing = server.create_named_project(("Billing", "BIL"), "billing", Some(BILLING_REMOTE));
  let invite = server.project_email_invite(EMAIL, &billing);
  server.serve_self_signed();
  let address = ServerAddress::parse(&server.url()).expect("address");
  let pin = server
    .block_on(Client::presented_certificate(&address))
    .expect("certificate");
  cloud.name_server(&audience, &server.url(), Some(&pin.to_hex()));
  cloud.invite(&invite, Some(("Billing", &format!("{}/billing", server.url()))));

  let mut window = window(None);
  let services = window.launch.services().expect("services").clone();
  let cloud_address = ServerAddress::parse(&cloud.url()).expect("cloud address");
  block_on(&window, services.cloud.set_address(Some(cloud_address))).expect("a test cloud");
  go(
    &mut window,
    Route::Orchester(OrchesterView::default()),
    "orchester-sign-in-form",
  );
  window.set("orchester-sign-in-email", EMAIL);
  window.set("orchester-sign-in-password", "not the password");
  press(&mut window, "orchester-sign-in");
  shows(&mut window, "orchester-sign-in-wrong");
  window.set("orchester-sign-in-password", PASSWORD);
  press(&mut window, "orchester-sign-in");
  let accept = format!("orchester-invite-{invite}-accept");
  shows(&mut window, &accept);
  assert!(window.text("orchester-row-cloud").contains("1 invite waiting"));
  press(&mut window, &accept);
  shows(&mut window, "orchester-invite-joined");

  let entry = services.servers.entries().expect("entries").remove(0);
  assert_eq!(
    entry.stored.certificate_pin,
    Some(pin.to_hex()),
    "pinned from the invite, nothing compared"
  );
  assert_eq!(
    entry.stored.joined_with,
    Some(JoinedWith::Cloud(audience.parse().expect("server id")))
  );
}

#[test]
fn orchester_connects_with_an_invite_code_after_the_check_code() {
  let mut server = TestServer::start();
  server.create_user("alice", "Alice Moreau", true);
  let kontur = server.create_named_project(("Kontur", "KTR"), "kontur", Some(KONTUR_REMOTE));
  let code = server.project_code(&kontur);
  server.serve_self_signed();
  let address = ServerAddress::parse(&server.url()).expect("address");
  let pin = server
    .block_on(Client::presented_certificate(&address))
    .expect("certificate");

  let mut window = window(None);
  go(
    &mut window,
    Route::Orchester(OrchesterView::connect()),
    "orchester-connect-address",
  );
  window.set("orchester-connect-address", &server.url());
  press(&mut window, "orchester-connect-continue");
  shows(&mut window, "orchester-connect-check-code-text");
  assert_eq!(window.text("orchester-connect-check-code-text"), pin.check_code());
  assert!(
    window.tree.get_element_by_id("orchester-connect-code").is_none(),
    "nothing asked before the check code"
  );
  press(&mut window, "orchester-connect-matches");
  shows(&mut window, "orchester-connect-code");
  window.set("orchester-connect-code", code.expose());
  window.set("orchester-connect-username", "ivan");
  window.set("orchester-connect-name", "Ivan Zaida");
  press(&mut window, "orchester-connect-join");
  shows(&mut window, "orchester-connect-joined");

  let services = window.launch.services().expect("services");
  let entry = services.servers.entries().expect("entries").remove(0);
  assert_eq!(entry.stored.certificate_pin, Some(pin.to_hex()));
  assert_eq!(entry.stored.joined_with, Some(JoinedWith::Code));
  assert_eq!(entry.stored.account.expect("account").username, "ivan");
}

/// A plain server behind a recording port, with Kontur and Billing; Ivan joined from the window's computer through
/// the port, a member of both.
fn joined_through_recorder(window: &support::Window) -> (TestServer, Recorder) {
  let server = TestServer::start();
  server.create_user("alice", "Alice Moreau", true);
  let kontur = server.create_named_project(("Kontur", "KON"), "kontur", Some(KONTUR_REMOTE));
  let billing = server.create_named_project(("Billing", "BIL"), "billing", Some(BILLING_REMOTE));
  let code = server.project_code(&kontur);
  let recorder = Recorder::start(server.address);
  let services = window.launch.services().expect("services").clone();
  let address = ServerAddress::parse(&recorder.url()).expect("address");
  let request = CodeRequest {
    code,
    username: Some("ivan".into()),
    display_name: Some("Ivan Zaida".into()),
  };
  block_on(
    window,
    services
      .servers
      .join_with_code(&address, Trust::System(PlainHttp::NotAllowed), request),
  )
  .expect("join with a code");
  server.add_member(&billing, "ivan");
  (server, recorder)
}

#[test]
fn orchester_links_by_address_warning_on_a_remote_mismatch() {
  let mut window = window(Some(KONTUR_REMOTE));
  let (_server, recorder) = joined_through_recorder(&window);
  go(
    &mut window,
    Route::Project(support::project_id(), ProjectPage::Settings),
    "settings-tab-sharing",
  );
  press(&mut window, "settings-tab-sharing");
  shows(&mut window, "sharing-address");
  window.set("sharing-address", &format!("{}/billing", recorder.url()));
  press(&mut window, "sharing-link-button");
  shows(&mut window, "sharing-mismatch");
  assert!(window.text("sharing-mismatch").contains("github.com/acme/kontur"));
  press(&mut window, "sharing-choose-another");
  window.set("sharing-address", &format!("{}/kontur", recorder.url()));
  press(&mut window, "sharing-link-button");
  shows(&mut window, "sharing-upload");
  assert!(
    window.tree.get_element_by_id("sharing-mismatch").is_none(),
    "the same remote does not warn"
  );
  press(&mut window, "sharing-link-confirm");
  shows(&mut window, "sharing-shared");
  assert!(
    !recorder.lines().iter().any(|line| administrative(line)),
    "an admin endpoint was called: {:?}",
    recorder.lines()
  );
}

#[test]
fn orchester_pairs_a_second_computer_and_signing_out_names_the_projects() {
  let mut window = window(Some(KONTUR_REMOTE));
  let (server, recorder) = joined_through_recorder(&window);
  let services = window.launch.services().expect("services").clone();
  let entry = services.servers.entries().expect("entries").remove(0);
  let request = orchester_workspace::sharing::project::LinkFirst {
    slug: "kontur".into(),
    remote: None,
    key: orchester_core::id::IdempotencyKey::generate(),
    write_discovery: false,
  };
  let root = window.data.repo("repo-kontur");
  block_on(&window, services.servers.link_first(&entry, &root, request)).expect("linked");

  go(
    &mut window,
    Route::Orchester(OrchesterView::server(entry.stored.id)),
    "orchester-server-add-computer",
  );
  press(&mut window, "orchester-server-add-computer");
  shows(&mut window, "orchester-pairing-code-text");
  let code = window.text("orchester-pairing-code-text");
  let other = Client::new(
    ServerAddress::parse(&server.url()).expect("address"),
    PlainHttp::NotAllowed,
  )
  .expect("client");
  let joined = server
    .block_on(other.join_with_code(&CodeJoin {
      code: Credential::new(code),
      device: "IVAN-LAPTOP · Windows".into(),
      username: None,
      display_name: None,
    }))
    .expect("the second computer joins with the pairing code");
  assert_eq!(joined.user.username, "ivan");

  press(&mut window, "orchester-pairing-done");
  press(&mut window, "orchester-server-sign-out");
  shows(&mut window, "orchester-server-sign-out-affects");
  assert!(window.text("orchester-server-sign-out-affects").contains("Kontur"));
  press(&mut window, "orchester-server-sign-out-confirm");
  window.until("signed out", |_| {
    services
      .servers
      .entries()
      .is_ok_and(|entries| entries[0].stored.account.is_none())
  });
  assert!(
    !recorder.lines().iter().any(|line| administrative(line)),
    "an admin endpoint was called: {:?}",
    recorder.lines()
  );
  assert!(recorder.lines().iter().any(|line| line == "POST /api/v1/auth/pairings"));
}
