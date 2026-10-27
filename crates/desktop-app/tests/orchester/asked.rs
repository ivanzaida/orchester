//! Routes that open Settings › Orchester Cloud's questions in place and *Connect a server* at a step (ORC-104), as a
//! QA copy's `navigate` does: *Decline the invite?*, *Sign out of Orchester Cloud?*, a server's *Sign out of …?* and
//! the check code to compare. Opening one is going somewhere; synthetic input still can't answer it.

use lurq::app::events::with_synthetic_input;
use orchester_api::client::ServerAddress;
use orchester_app::routes::{ConnectStep, OrchesterAsk, OrchesterPanel, OrchesterView, Route};
use orchester_workspace::sharing::{CodeRequest, trust::PresentedCertificate};

use super::{
  KONTUR_REMOTE,
  account_cloud::{AccountCloud, EMAIL},
  harness::{block_on, go, shows, window},
  joined_through_recorder,
  server_support::TestServer,
  signed_in,
  support::Window,
};

/// Presses `id` with synthetic input, as an MCP client would, and finds it refused under the button.
fn refused(window: &mut Window, id: &str) {
  shows(window, id);
  with_synthetic_input(|| window.press(id));
  shows(window, &format!("{id}-person-only"));
}

#[test]
fn orchester_asked_decline_and_sign_out_open_in_place() {
  let cloud = AccountCloud::start();
  let server = TestServer::start();
  server.create_user("alice", "Alice Moreau", true);
  let audience = server.link_cloud(&cloud.root_key, &cloud.key_set);
  cloud.name_server(&audience, &server.url(), None);
  let invite = server.email_invite(EMAIL);
  cloud.invite(&invite, None);
  let mut window = signed_in(&cloud);
  let card = format!("orchester-invite-{invite}");
  let decline = OrchesterView {
    ask: Some(OrchesterAsk::Decline(invite.parse().expect("an invite id"))),
    ..OrchesterView::default()
  };
  go(
    &mut window,
    Route::Orchester(decline),
    &format!("{card}-decline-question"),
  );
  refused(&mut window, &format!("{card}-decline-confirm"));
  assert!(cloud.declined().is_empty(), "nothing was declined");

  let sign_out = OrchesterView {
    ask: Some(OrchesterAsk::SignOut),
    ..OrchesterView::default()
  };
  go(
    &mut window,
    Route::Orchester(sign_out),
    "orchester-cloud-sign-out-question",
  );
  refused(&mut window, "orchester-cloud-sign-out-confirm");
  assert!(!cloud.signed_out(), "still signed in");
}

#[test]
fn orchester_asked_a_servers_sign_out_opens_on_its_page() {
  let mut window = window(Some(KONTUR_REMOTE));
  let (_server, _recorder) = joined_through_recorder(&window);
  let services = window.launch.services().expect("services").clone();
  let entry = services.servers.entries().expect("entries").remove(0);
  let view = OrchesterView {
    ask: Some(OrchesterAsk::SignOut),
    ..OrchesterView::server(entry.stored.id)
  };
  go(
    &mut window,
    Route::Orchester(view),
    "orchester-server-sign-out-question",
  );
  refused(&mut window, "orchester-server-sign-out-confirm");
  assert!(
    services.servers.entries().expect("entries")[0].in_use(),
    "still signed in"
  );
}

#[test]
fn orchester_connect_opens_at_the_check_code_and_stops_there() {
  let mut server = TestServer::start();
  server.create_user("alice", "Alice Moreau", true);
  let code = server.code_for("ivan", false);
  server.serve_self_signed();
  let mut window = window(None);
  let services = window.launch.services().expect("services").clone();
  let address = ServerAddress::parse(&server.url()).expect("address");
  let presented = block_on(&window, PresentedCertificate::read(&address)).expect("its certificate");
  let check = presented.check_code();
  let request = CodeRequest {
    code,
    username: Some("ivan".into()),
    display_name: Some("Ivan Zaida".into()),
    proof: Some(server.code_proof("ivan")),
  };
  let id = block_on(
    &window,
    services.servers.join_with_code(&address, presented.confirm(), request),
  )
  .expect("joined");
  let entry = services.servers.entries().expect("entries").remove(0);
  block_on(&window, services.servers.sign_out(&entry)).expect("signed out here");
  for step in [ConnectStep::Compare, ConnectStep::Join] {
    let view = OrchesterView {
      server: Some(id),
      panel: OrchesterPanel::Connect,
      step,
      ask: None,
    };
    go(&mut window, Route::Orchester(view), "orchester-connect-check-code-text");
    assert_eq!(window.text("orchester-connect-check-code-text"), check);
    // The done *Address* step is one line, 50 high, and the code is Geist Mono 16 (`Vn33B`, `UWPnK` in `IBnMu`).
    let mut height = |id: &str| {
      window
        .tree
        .get_element_by_id_mut(id)
        .and_then(|element| element.bounds())
        .unwrap_or_else(|| panic!("{id} is laid out"))
        .height
    };
    let address = height("orchester-connect-step-address");
    assert!(
      (address - 50.0).abs() < 1.0,
      "{step:?}: the done address step is {address} high"
    );
    let code = height("orchester-connect-check-code-text");
    assert!(code < 24.0, "{step:?}: the check code is {code} high");
    assert!(
      window.tree.get_element_by_id("orchester-connect-code").is_none(),
      "{step:?}: Join waits for a person to compare the code"
    );
    go(&mut window, Route::AllProjects, "all-projects-title");
  }
}
