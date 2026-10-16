//! N1 security review 1, M2: an invite accepted here is kept until its join finished. A join that failed after the
//! cloud issued its token is tried again from *Waiting invites*, with a new token; a join that waits for approval
//! shows *Waiting for approval* and finishes once an admin approves it. N1 design 2: the check says the join was
//! approved (`P0wvD`) or refused (`S5mMY`); one no longer asked on its own, or asked as often as Orchester does,
//! says so.

use orchester_app::routes::{OrchesterView, Route};
use orchester_core::time::Timestamp;
use orchester_store::{AcceptedInvite, AppDb};

use super::{
  account_cloud::{AccountCloud, EMAIL},
  harness::{Recorder, block_on, go, press, shows},
  server_support::TestServer,
  signed_in,
  support::Window,
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
  window.until("nothing left to finish", |window| {
    window.tree.get_element_by_id(&kept).is_none()
  });
}

/// The window signed in to `cloud`, after its account joined `server` once and signed out, with an invite to Billing
/// accepted that waits for approval; the invite's id.
fn waiting_join(cloud: &AccountCloud, (server, address): (&TestServer, &str)) -> (Window, String) {
  // The account joins from this computer once, then signs out: the server knows it from now on.
  let first = server.email_invite(EMAIL);
  cloud.invite(&first, None);
  let mut window = signed_in(cloud);
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
  shows(&mut window, &format!("orchester-accepted-{second}-title"));
  (window, second)
}

/// Changes what the window's database keeps of its one accepted invite.
fn rekeep(window: &Window, change: impl FnOnce(AcceptedInvite) -> AcceptedInvite) {
  let mut db = AppDb::open(&window.data.database()).expect("the window's database");
  let kept = db.accepted_invites(Timestamp::now()).expect("kept").remove(0);
  db.keep_accepted_invite(&change(kept)).expect("kept again");
}

/// Makes the check's next ask due and opens Settings › Orchester again, which checks when it opens.
fn check_now(window: &mut Window) {
  let due = Timestamp::from_unix_millis(Timestamp::now().unix_millis() - 1000);
  rekeep(window, |kept| AcceptedInvite {
    next_check_at: Some(due),
    ..kept
  });
  go(window, Route::AllProjects, "all-projects-tabs");
  go(
    window,
    Route::Orchester(OrchesterView::default()),
    "orchester-cloud-details",
  );
}

#[test]
fn orchester_accept_never_lists_an_invite_it_keeps_as_accepted() {
  let cloud = AccountCloud::start();
  let (server, recorder, _) = linked(&cloud, true);
  let recorder = recorder.expect("a recording port");
  let invite = server.email_invite(EMAIL);
  cloud.invite(&invite, None);
  let mut window = signed_in(&cloud);
  // The server cannot be reached before the cloud issued a token: the cloud still lists the invite (N1 design 3).
  recorder.refuse_once("GET /api/v1/server");
  press(&mut window, &format!("orchester-invite-{invite}-accept"));
  let kept = format!("orchester-accepted-{invite}");
  shows(&mut window, &format!("{kept}-stopped"));
  assert_eq!(cloud.tokens_for(&invite), 0, "no token was issued");
  window.until("listed as accepted only", |window| {
    window
      .tree
      .get_element_by_id(&format!("orchester-invite-{invite}"))
      .is_none()
  });
  shows(&mut window, "orchester-invites-none");
  assert!(
    !window.text("orchester-invites-title").contains("waiting"),
    "{}",
    window.text("orchester-invites-title")
  );
  go(&mut window, Route::AllProjects, "all-projects-tabs");
  window.pass();
  assert!(
    window
      .tree
      .get_element_by_id(&format!("all-projects-invite-{invite}"))
      .is_none(),
    "the All projects notice does not offer it either"
  );
}

#[test]
fn orchester_accept_waits_for_approval_and_finishes_once_approved() {
  let cloud = AccountCloud::start();
  let (server, _, address) = linked(&cloud, false);
  let (mut window, second) = waiting_join(&cloud, (&server, &address));
  let services = window.launch.services().expect("services").clone();
  let kept = format!("orchester-accepted-{second}");
  assert_eq!(window.text(&format!("{kept}-title")), "Waiting for approval");
  assert!(!services.servers.entries().expect("entries")[0].in_use(), "not in yet");

  assert_eq!(server.approve_waiting(), 1, "an admin approves this computer");
  check_now(&mut window);
  let ended = format!("orchester-ended-{second}");
  shows(&mut window, &format!("{ended}-approved"));
  assert!(
    window
      .text(&format!("{ended}-approved"))
      .contains("approved this computer"),
    "{}",
    window.text(&format!("{ended}-approved"))
  );
  window.until("the waiting card gone", |window| {
    window.tree.get_element_by_id(&kept).is_none()
  });
  let entry = services.servers.entries().expect("entries").remove(0);
  assert!(entry.in_use(), "joined");
  assert!(
    block_on(&window, services.cloud.accepted_invites())
      .expect("kept")
      .is_empty()
  );
  press(&mut window, &format!("{ended}-continue"));
  shows(&mut window, "orchester-server-details");
  assert!(window.tree.get_element_by_id(&ended).is_none(), "put away");
}

#[test]
fn orchester_accept_says_when_the_server_refused_the_computer() {
  let cloud = AccountCloud::start();
  let (server, _, address) = linked(&cloud, false);
  let (mut window, second) = waiting_join(&cloud, (&server, &address));
  assert_eq!(server.refuse_waiting(), 1, "an admin refuses this computer");
  check_now(&mut window);
  let ended = format!("orchester-ended-{second}");
  shows(&mut window, &format!("{ended}-refused"));
  assert_eq!(
    window.text(&format!("{ended}-refused")),
    format!(
      "{name} refused this computer Nothing was uploaded. If you didn't expect this, ask an admin of {name}; if \
       someone else refused it, your Orchester password may be known to them.",
      name = "Kontur team"
    )
  );
  let services = window.launch.services().expect("services").clone();
  assert!(
    block_on(&window, services.cloud.accepted_invites())
      .expect("kept")
      .is_empty(),
    "a refused invite is not asked for again"
  );
  press(&mut window, &format!("{ended}-back"));
  window.until("put away", |window| window.tree.get_element_by_id(&ended).is_none());
}

#[test]
fn orchester_accept_stops_asking_and_says_why() {
  let cloud = AccountCloud::start();
  let (server, _, address) = linked(&cloud, false);
  let (mut window, second) = waiting_join(&cloud, (&server, &address));
  let kept = format!("orchester-accepted-{second}");
  // The backoff ran out: the person tries again.
  rekeep(&window, |kept| AcceptedInvite {
    next_check_at: None,
    ..kept
  });
  go(&mut window, Route::AllProjects, "all-projects-tabs");
  go(
    &mut window,
    Route::Orchester(OrchesterView::default()),
    &format!("{kept}-try-again"),
  );
  assert!(
    window
      .text(&format!("{kept}-stopped"))
      .contains("stopped asking on its own"),
    "{}",
    window.text(&format!("{kept}-stopped"))
  );
  // Asked as often as Orchester does: no Try again, Cancel joining instead.
  rekeep(&window, |kept| AcceptedInvite { asks: 10, ..kept });
  go(&mut window, Route::AllProjects, "all-projects-tabs");
  go(
    &mut window,
    Route::Orchester(OrchesterView::default()),
    &format!("{kept}-cancel"),
  );
  assert!(window.tree.get_element_by_id(&format!("{kept}-try-again")).is_none());
  assert!(
    window.text(&format!("{kept}-stopped")).contains("10 times"),
    "{}",
    window.text(&format!("{kept}-stopped"))
  );
  press(&mut window, &format!("{kept}-cancel"));
  window.until("set aside", |window| window.tree.get_element_by_id(&kept).is_none());
}
