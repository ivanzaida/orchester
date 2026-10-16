//! N1 security review 1, M2: an invite accepted here is kept until its join finished. A join that failed after the
//! cloud issued its token is tried again from *Waiting invites*, with a new token; a join that waits for approval
//! shows *Waiting for approval* and finishes once an admin approves it.

use orchester_app::routes::{OrchesterView, Route};

use super::{
  account_cloud::{AccountCloud, EMAIL},
  harness::{Recorder, block_on, go, press, shows},
  server_support::TestServer,
  signed_in,
};

/// A plain team server linked to the stand-in `cloud`, which names it at `address` (`None`: its own).
fn linked(cloud: &AccountCloud, behind: bool) -> (TestServer, Option<Recorder>, String) {
  let server = TestServer::start();
  server.create_user("alice", "Alice Moreau", true);
  let audience = server.link_cloud(&cloud.root_key, &cloud.key_set);
  let recorder = behind.then(|| Recorder::start(server.address));
  let address = recorder.as_ref().map_or_else(|| server.url(), Recorder::url);
  cloud.name_server(&audience, &address, None);
  (server, recorder, address)
}

#[test]
fn orchester_accept_retries_a_join_that_failed_after_its_token() {
  let cloud = AccountCloud::start();
  let (server, recorder, _) = linked(&cloud, true);
  let recorder = recorder.expect("a recording port");
  let invite = server.email_invite(EMAIL);
  cloud.invite(&invite, None);
  let mut window = signed_in(&cloud);
  let services = window.launch.services().expect("services").clone();
  recorder.refuse_once("POST /api/v1/auth/cloud");
  press(&mut window, &format!("orchester-invite-{invite}-accept"));
  let kept = format!("orchester-accepted-{invite}");
  shows(&mut window, &format!("{kept}-stopped"));
  assert_eq!(
    cloud.tokens_for(&invite),
    1,
    "the token was issued before the join failed"
  );
  assert!(
    services.servers.entries().expect("entries").is_empty(),
    "nothing joined"
  );

  press(&mut window, &format!("{kept}-try-again"));
  shows(&mut window, "orchester-invite-finish-joined");
  assert_eq!(cloud.tokens_for(&invite), 2, "asked again with a new token");
  assert!(services.servers.entries().expect("entries")[0].in_use(), "joined");
  assert!(
    block_on(&window, services.cloud.accepted_invites())
      .expect("kept")
      .is_empty()
  );
  assert!(window.tree.get_element_by_id(&kept).is_none(), "nothing left to finish");
}

#[test]
fn orchester_accept_waits_for_approval_and_finishes_once_approved() {
  let cloud = AccountCloud::start();
  let (server, _, address) = linked(&cloud, false);
  // The account joins from this computer once, then signs out: the server knows it from now on.
  let first = server.email_invite(EMAIL);
  cloud.invite(&first, None);
  let mut window = signed_in(&cloud);
  let services = window.launch.services().expect("services").clone();
  let listed = block_on(&window, services.cloud.invites()).expect("invites").remove(0);
  block_on(&window, services.cloud.accept(&listed)).expect("joined");
  let entry = services.servers.entries().expect("entries").remove(0);
  block_on(&window, services.servers.sign_out(&entry)).expect("signed out");

  server.set_join_approval(true);
  let billing = server.create_named_project(("Billing", "BIL"), "billing", None);
  let second = server.project_email_invite(EMAIL, &billing);
  cloud.invite(&second, Some(("Billing", &format!("{address}/billing"))));
  go(
    &mut window,
    Route::Orchester(OrchesterView::default()),
    &format!("orchester-invite-{second}-accept"),
  );
  press(&mut window, &format!("orchester-invite-{second}-accept"));
  let kept = format!("orchester-accepted-{second}");
  shows(&mut window, &format!("{kept}-title"));
  assert_eq!(window.text(&format!("{kept}-title")), "Waiting for approval");
  assert!(!services.servers.entries().expect("entries")[0].in_use(), "not in yet");

  assert_eq!(server.approve_waiting(), 1, "an admin approves this computer");
  let second_id = second.parse().expect("invite id");
  block_on(&window, services.cloud.finish_accept(second_id)).expect("joined once approved");
  window.until("the waiting card gone", |window| {
    window.tree.get_element_by_id(&kept).is_none()
  });
  assert!(services.servers.entries().expect("entries")[0].in_use(), "joined");
  assert!(
    block_on(&window, services.cloud.accepted_invites())
      .expect("kept")
      .is_empty()
  );
}
