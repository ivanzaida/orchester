//! Settings › Orchester, *Connect a server* and Project settings › Sharing (N1) in the window, against a real team
//! server and a stand-in for Orchester Cloud's account API on loopback: signing in, the cloud's pages opened in the
//! browser, accepting an invite with the certificate pin it carries, connecting with an invite code after the check
//! code, linking a local project by address with and without a remote mismatch, a pairing code for a second computer,
//! signing out naming the projects it affects, the server's version kept while its projects are read again, and that
//! the app never calls an endpoint of the server admin. The sign-in against the real cloud is in
//! `crates/cloud/tests/cloud/desktop.rs` in `ivanzaida/orchester-cloud`.
//!
//! `cargo test -p orchester-app --test orchester orchester_`

mod accepting;
mod asked;
mod clipboard;
mod design;
mod harness;
mod history;
mod invites;
mod landing;
mod person_only;
#[path = "../../../workspace/tests/server_support/mod.rs"]
mod server_support;
#[path = "../support/mod.rs"]
mod support;
mod two_factor;

use harness::{Recorder, administrative, block_on, go, press, shows, window};
use orchester_api::{
  CodeJoin, Credential,
  client::{Client, PlainHttp, ServerAddress},
};
use orchester_app::routes::{OrchesterView, ProjectPage, Route};
use orchester_store::JoinedWith;
use orchester_workspace::sharing::{CodeRequest, Trust, cloud::BUILD_ADDRESS};
use server_support::TestServer;
use support::signed_in::account_cloud::{self, AccountCloud, EMAIL, PASSWORD};

const KONTUR_REMOTE: &str = "https://github.com/acme/kontur.git";
const BILLING_REMOTE: &str = "https://github.com/acme/billing.git";

#[test]
fn orchester_web_links_open_the_clouds_pages() {
  let mut window = window(None);
  let services = window.launch.services().expect("services").clone();
  // Back to the build's own cloud, which signs out: the gate holds its sign-in and its pages.
  block_on(&window, services.cloud.set_address(None)).expect("the build's cloud");
  shows(&mut window, "gate-create-account");
  press(&mut window, "gate-create-account");
  press(&mut window, "gate-forgot");
  let links = window.launch.services().expect("services").links.recorded();
  assert_eq!(
    links,
    vec![format!("{BUILD_ADDRESS}/signup"), format!("{BUILD_ADDRESS}/forgot")]
  );
}

#[test]
fn orchester_signs_in_and_accepts_an_invite_with_its_pin() {
  let cloud = AccountCloud::start();
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
  // Another cloud signs out: the sign-in is the gate's, a person's.
  block_on(&window, services.cloud.set_address(Some(cloud_address))).expect("a test cloud");
  shows(&mut window, "gate-email");
  window.set("gate-email", EMAIL);
  window.set("gate-password", "not the password");
  press(&mut window, "gate-sign-in");
  shows(&mut window, "gate-wrong");
  window.set("gate-password", PASSWORD);
  press(&mut window, "gate-sign-in");
  let accept = format!("orchester-invite-{invite}-accept");
  go(&mut window, Route::Orchester(OrchesterView::default()), &accept);
  // The invite is a row of *Invites*, its heading counting what waits (`YwJ2b` in `ZUuMT`).
  assert!(window.text("orchester-invites-meta").contains("1 waiting"));
  invite_notice(&mut window, &invite);
  go(&mut window, Route::Orchester(OrchesterView::default()), &accept);
  press(&mut window, &accept);
  // A project invite goes on to which checkout to link: Billing has none here, so its clone is offered.
  shows(&mut window, "orchester-landing-none");
  username_suggested(&mut window);

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

/// All projects' notice names the invite without its role, which its second line says, and asks *Decline* in place
/// with the notice's buttons put away.
fn invite_notice(window: &mut support::Window, invite: &str) {
  let notice = format!("all-projects-invite-{invite}");
  go(window, Route::AllProjects, &notice);
  let title = window.text(&format!("{notice}-title"));
  assert!(!title.contains("as a member"), "the title leaves the role out: {title}");
  assert!(window.text(&format!("{notice}-body")).starts_with("As a member"));
  press(window, &format!("{notice}-decline"));
  shows(window, &format!("{notice}-decline-question"));
  assert!(window.tree.get_element_by_id(&format!("{notice}-accept")).is_none());
  press(window, &format!("{notice}-keep"));
  shows(window, &format!("{notice}-accept"));
}

/// Joining another server by code starts from the signed-in account's username and name.
fn username_suggested(window: &mut support::Window) {
  let other = TestServer::start();
  other.create_user("alice", "Alice Moreau", true);
  go(
    window,
    Route::Orchester(OrchesterView::connect()),
    "orchester-connect-address",
  );
  window.set("orchester-connect-address", &other.url());
  press(window, "orchester-connect-continue");
  shows(window, "orchester-connect-username");
  assert_eq!(window.text("orchester-connect-username"), "bohdan");
  assert_eq!(window.text("orchester-connect-name"), "Bohdan Koval");
}

#[test]
fn orchester_connects_with_an_invite_code_after_the_check_code() {
  let mut server = TestServer::start();
  server.create_user("alice", "Alice Moreau", true);
  let kontur = server.create_named_project(("Kontur", "KTR"), "kontur", Some(KONTUR_REMOTE));
  let code = server.project_code(&kontur);
  // Its admin pins the tests' cloud root, so the server checks the two-factor identity a code needs.
  server.cloud_audience();
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
  // The join review the design opens next is N3's: in place, unavailable, beside the server's entry.
  shows(&mut window, "orchester-connect-join-review");
  shows(&mut window, "orchester-connect-open-joined");

  let services = window.launch.services().expect("services");
  let entry = services.servers.entries().expect("entries").remove(0);
  assert_eq!(entry.stored.certificate_pin, Some(pin.to_hex()));
  assert_eq!(entry.stored.joined_with, Some(JoinedWith::Code));
  assert_eq!(entry.stored.account.expect("account").username, "ivan");
}

#[test]
fn orchester_refuses_a_code_without_a_two_factor_sign_in() {
  let mut server = TestServer::start();
  server.create_user("alice", "Alice Moreau", true);
  let kontur = server.create_named_project(("Kontur", "KTR"), "kontur", Some(KONTUR_REMOTE));
  let code = server.project_code(&kontur);
  // Its admin pins the tests' cloud root, so the server checks the two-factor identity a code needs.
  server.cloud_audience();
  server.serve_self_signed();
  let mut window = window(None);
  window.data.cloud.two_factor_off();
  let services = window.launch.services().expect("services").clone();
  go(
    &mut window,
    Route::Orchester(OrchesterView::connect()),
    "orchester-connect-address",
  );
  window.set("orchester-connect-address", &server.url());
  press(&mut window, "orchester-connect-continue");
  press(&mut window, "orchester-connect-matches");
  shows(&mut window, "orchester-connect-code");
  window.set("orchester-connect-code", code.expose());
  window.set("orchester-connect-username", "olena");
  window.set("orchester-connect-name", "Olena Koval");
  press(&mut window, "orchester-connect-join");
  shows(&mut window, "orchester-connect-join-error");
  assert!(
    window
      .text("orchester-connect-join-error")
      .starts_with("Entering a code needs two-factor authentication on your Orchester Cloud account."),
    "{}",
    window.text("orchester-connect-join-error")
  );
  assert!(
    services.servers.entries().expect("entries").is_empty(),
    "the code was not sent"
  );
}

/// Links the window's Kontur to Kontur on the one server it joined, and gives that server's entry.
fn link_kontur(window: &support::Window) -> orchester_workspace::sharing::ServerEntry {
  let services = window.launch.services().expect("services").clone();
  let entry = services.servers.entries().expect("entries").remove(0);
  let request = orchester_workspace::sharing::project::LinkFirst {
    slug: "kontur".into(),
    remote: None,
    key: orchester_core::id::IdempotencyKey::generate(),
    write_discovery: false,
  };
  let root = window.data.repo("repo-kontur");
  block_on(window, services.servers.link_first(&entry, &root, request)).expect("linked");
  entry
}

/// The window signed in to the stand-in `cloud`, at Settings › Orchester.
fn signed_in(cloud: &AccountCloud) -> support::Window {
  signed_in_with(cloud, None)
}

/// [`signed_in`] with Kontur's checkout's `origin` set to `origin`.
fn signed_in_with(cloud: &AccountCloud, origin: Option<&str>) -> support::Window {
  let mut window = window(origin);
  let services = window.launch.services().expect("services").clone();
  let cloud_address = ServerAddress::parse(&cloud.url()).expect("cloud address");
  block_on(&window, services.cloud.set_address(Some(cloud_address))).expect("a test cloud");
  block_on(
    &window,
    services.cloud.sign_in(EMAIL.into(), Credential::new(PASSWORD.into())),
  )
  .expect("signed in");
  go(
    &mut window,
    Route::Orchester(OrchesterView::default()),
    "orchester-cloud-details",
  );
  window
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
    proof: Some(server.code_proof("ivan")),
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
    Route::Project(support::project_id(), ProjectPage::Settings(Default::default())),
    "settings-tab-sharing",
  );
  press(&mut window, "settings-tab-sharing");
  press(&mut window, "sharing-share-open");
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
  shows(&mut window, "sharing-role-value");
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
  let entry = link_kontur(&window);

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
  let proof = server.code_proof("ivan");
  let joined = server
    .block_on(other.join_with_code(&CodeJoin {
      code: Credential::new(code),
      device: "IVAN-LAPTOP · Windows".into(),
      username: None,
      display_name: None,
      kind: orchester_api::DeviceKind::App,
      identity_token: Some(proof.identity_token),
      key_set: proof.key_set,
    }))
    .expect("the second computer joins with the pairing code");
  assert_eq!(joined.user.username, "ivan");

  press(&mut window, "orchester-pairing-done");
  harness::sign_out_of_server(&mut window);
  shows(&mut window, "orchester-server-sign-out-affects");
  assert!(window.text("orchester-server-sign-out-affects").contains("Kontur"));
  press(&mut window, "orchester-server-sign-out-confirm");
  window.until("signed out", |_| {
    services
      .servers
      .entries()
      .is_ok_and(|entries| entries[0].stored.account.is_none())
  });
  window.until("the token note of a server signed out", |window| {
    window
      .texts("orchester-server-token-note")
      .is_some_and(|texts| texts.join(" ").contains("holds no token"))
  });
  assert!(
    !recorder.lines().iter().any(|line| administrative(line)),
    "an admin endpoint was called: {:?}",
    recorder.lines()
  );
  assert!(recorder.lines().iter().any(|line| line == "POST /api/v1/auth/pairings"));
}

#[test]
fn orchester_server_keeps_its_version_as_its_projects_are_read_again() {
  let mut window = window(Some(KONTUR_REMOTE));
  let (server, recorder) = joined_through_recorder(&window);
  let services = window.launch.services().expect("services").clone();
  let entry = services.servers.entries().expect("entries").remove(0);
  go(
    &mut window,
    Route::Orchester(OrchesterView::server(entry.stored.id)),
    "orchester-server-projects-rows",
  );
  window.until("the server's version", version_shown);
  let asked = version_reads(&recorder);
  let notes = server.create_named_project(("Notes", "NOT"), "notes", None);
  server.add_member(&notes, "ivan");

  // Asking the server again now reads the projects again.
  services.servers.retry_now(entry.stored.id);
  let mut kept = true;
  window.until("the projects read again", |window| {
    kept &= version_shown(window);
    window
      .texts("orchester-server-projects-rows")
      .is_some_and(|texts| texts.iter().any(|text| text == "Notes"))
  });
  assert!(
    kept,
    "the version stays in the header while the projects are read again"
  );
  assert_eq!(version_reads(&recorder), asked, "the version is not asked again");
}

/// Whether the header says what the server runs, `Orchester server 0.4.0, API 3`.
fn version_shown(window: &mut support::Window) -> bool {
  window
    .texts("orchester-server-header-facts")
    .is_some_and(|texts| texts.iter().any(|text| text.starts_with("Orchester server")))
}

/// How many times the app asked the server what it runs (`GET /server`).
fn version_reads(recorder: &Recorder) -> usize {
  recorder
    .lines()
    .iter()
    .filter(|line| *line == "GET /api/v1/server")
    .count()
}
