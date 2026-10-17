//! Two-factor authentication in Settings › Orchester and the All projects notice (layer `uyO5v`: `hC11x`, `rPG8h`,
//! board `qwDH9`), against the stand-in cloud: the header says whether it is on; with it off, an invite offers *Turn on
//! two-factor authentication*, which opens the cloud's security page; signed in without a code, the cloud's refusal
//! of an accept offers *Sign in again*, which signs out and locks, and the gate's sign-in with a code lets the invite
//! be accepted; and a code join works after a sign-in with a code.

use lurq::app::events::with_synthetic_input;
use orchester_app::routes::{OrchesterView, Route};

use super::{
  KONTUR_REMOTE,
  account_cloud::{AccountCloud, EMAIL, PASSWORD},
  harness::{block_on, go, press, shows, window},
  server_support::TestServer,
  signed_in,
  support::Window,
};

/// A plain team server linked to `cloud`, which names it, with Billing and an invite to it for the account; the server
/// and the invite's id.
fn invited(cloud: &AccountCloud) -> (TestServer, String) {
  let server = TestServer::start();
  server.create_user("alice", "Alice Moreau", true);
  let audience = server.link_cloud(&cloud.root_key, &cloud.key_set);
  let billing = server.create_named_project(("Billing", "BIL"), "billing", None);
  let invite = server.project_email_invite(EMAIL, &billing);
  cloud.name_server(&audience, &server.url(), None);
  cloud.invite(&invite, Some(("Billing", &format!("{}/billing", server.url()))));
  (server, invite)
}

fn recorded(window: &Window) -> Vec<String> {
  window.launch.services().expect("services").links.recorded()
}

#[test]
fn orchester_two_factor_off_offers_turning_it_on() {
  let cloud = AccountCloud::start();
  let (_server, invite) = invited(&cloud);
  cloud.two_factor_off();
  let mut window = signed_in(&cloud);
  let security = format!("{}/account#security", cloud.url());
  let card = format!("orchester-invite-{invite}");
  shows(&mut window, &format!("{card}-turn-on"));
  assert_eq!(window.text("orchester-cloud-two-factor"), "Two-factor: off");
  assert_eq!(
    window.text(&format!("{card}-two-factor")),
    "Accepting needs two-factor authentication on your account."
  );
  assert!(window.tree.get_element_by_id(&format!("{card}-accept")).is_none());
  assert!(window.tree.get_element_by_id(&format!("{card}-decline")).is_some());
  press(&mut window, "orchester-cloud-two-factor-turn-on");
  press(&mut window, &format!("{card}-turn-on"));
  assert_eq!(recorded(&window), vec![security.clone(), security]);

  let notice = format!("all-projects-invite-{invite}");
  go(&mut window, Route::AllProjects, &format!("{notice}-turn-on"));
  assert_eq!(
    window.text(&format!("{notice}-two-factor")),
    "Accepting needs two-factor authentication on your account."
  );
  assert!(
    window
      .text(&format!("{notice}-two-factor-note"))
      .starts_with("Turn on two-factor authentication opens")
  );
  press(&mut window, &format!("{notice}-turn-on"));
  assert_eq!(recorded(&window).len(), 3);
}

#[test]
fn orchester_two_factor_signs_in_again_with_a_code_and_accepts() {
  let cloud = AccountCloud::start();
  let (_server, invite) = invited(&cloud);
  let mut window = signed_in(&cloud);
  let services = window.launch.services().expect("services").clone();
  let card = format!("orchester-invite-{invite}");
  shows(&mut window, &format!("{card}-accept"));
  assert_eq!(window.text("orchester-cloud-two-factor"), "Two-factor: on");
  // Signed in before two-factor authentication was turned on: the listing above is from before.
  cloud.session_without_two_factor();
  cloud.require_codes();
  press(&mut window, &format!("{card}-accept"));
  shows(&mut window, &format!("{card}-sign-in-needed"));
  assert_eq!(
    window.text(&format!("{card}-sign-in-needed-title")),
    "Sign in again to accept"
  );
  assert_eq!(cloud.tokens_for(&invite), 0, "no token accepted it");
  press(&mut window, &format!("{card}-not-now"));
  shows(&mut window, &format!("{card}-two-factor"));
  assert_eq!(
    window.text(&format!("{card}-two-factor")),
    "Accepting needs a sign-in with your code. This computer signed in before you turned on two-factor."
  );
  go(
    &mut window,
    Route::AllProjects,
    &format!("all-projects-invite-{invite}-sign-in-again"),
  );

  go(
    &mut window,
    Route::Orchester(OrchesterView::default()),
    &format!("{card}-sign-in-again"),
  );
  with_synthetic_input(|| window.press(&format!("{card}-sign-in-again")));
  shows(&mut window, &format!("{card}-sign-in-again-person-only"));
  assert!(services.cloud.is_signed_in(), "synthetic input signs nothing out");
  window.press(&format!("{card}-sign-in-again"));
  shows(&mut window, "gate-notice");
  assert_eq!(
    window.text("gate-notice-title"),
    "Sign in with your code to accept invites"
  );
  assert_eq!(
    window.text("gate-notice-body"),
    "This computer signed in before you turned on two-factor. Sign in again; the invite to Billing waits in Settings \
     › Orchester."
  );
  assert!(cloud.signed_out(), "the cloud ended the session");

  window.set("gate-password", PASSWORD);
  press(&mut window, "gate-sign-in");
  shows(&mut window, "gate-code");
  let code = cloud.next_code();
  window.set("gate-code", &code);
  press(&mut window, "gate-verify");
  shows(&mut window, "content");
  go(
    &mut window,
    Route::Orchester(OrchesterView::default()),
    &format!("{card}-accept"),
  );
  press(&mut window, &format!("{card}-accept"));
  shows(&mut window, "orchester-invite-joined");
  assert!(services.servers.entries().expect("entries")[0].in_use(), "joined");
}

#[test]
fn orchester_two_factor_code_join_works_after_a_sign_in_with_a_code() {
  let mut server = TestServer::start();
  server.create_user("alice", "Alice Moreau", true);
  let kontur = server.create_named_project(("Kontur", "KTR"), "kontur", Some(KONTUR_REMOTE));
  let code = server.project_code(&kontur);
  // Its admin pins the tests' cloud root, so the server checks the two-factor identity a code needs.
  server.cloud_audience();
  server.serve_self_signed();
  let mut window = window(None);
  let cloud = &window.data.cloud;
  cloud.session_without_two_factor();
  cloud.require_codes();
  let services = window.launch.services().expect("services").clone();
  connect_with_code(&mut window, (&server.url(), code.expose()));
  shows(&mut window, "orchester-connect-join-error");
  assert!(
    window
      .text("orchester-connect-join-error")
      .starts_with("Entering a code needs this computer signed in to Orchester Cloud with your two-factor code."),
    "{}",
    window.text("orchester-connect-join-error")
  );
  assert!(
    services.servers.entries().expect("entries").is_empty(),
    "the code was not sent"
  );

  block_on(&window, services.cloud.sign_out()).expect("signed out");
  shows(&mut window, "gate-password");
  window.set("gate-password", PASSWORD);
  press(&mut window, "gate-sign-in");
  shows(&mut window, "gate-code");
  let next = window.data.cloud.next_code();
  window.set("gate-code", &next);
  press(&mut window, "gate-verify");
  shows(&mut window, "content");
  connect_with_code(&mut window, (&server.url(), code.expose()));
  shows(&mut window, "orchester-connect-joined");
  let entry = services.servers.entries().expect("entries").remove(0);
  assert_eq!(entry.stored.account.expect("account").username, "ivan");
}

/// Connects to the server at `address` with `code`, as Ivan, the check code compared.
fn connect_with_code(window: &mut Window, (address, code): (&str, &str)) {
  go(
    window,
    Route::Orchester(OrchesterView::connect()),
    "orchester-connect-address",
  );
  window.set("orchester-connect-address", address);
  press(window, "orchester-connect-continue");
  press(window, "orchester-connect-matches");
  shows(window, "orchester-connect-code");
  window.set("orchester-connect-code", code);
  window.set("orchester-connect-username", "ivan");
  window.set("orchester-connect-name", "Ivan Zaida");
  press(window, "orchester-connect-join");
}
