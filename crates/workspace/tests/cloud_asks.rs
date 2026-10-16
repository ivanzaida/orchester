//! How often an invite accepted here is asked for while its join waits for approval (N1 security re-review 2): each
//! ask takes an identity token, so a team server that answers *waiting* forever cannot use up the computer's tokens.
//! It is asked again on its own after 2 minutes, then 4, then 8, not after its approval expires, and no more than ten
//! times in all, the person's asks included.
//!
//! `cargo test -p orchester-workspace --test cloud_asks cloud_asks_`

#[path = "sharing_support/account_cloud.rs"]
mod account_cloud;
#[path = "sharing_support/desktop.rs"]
mod desktop;
#[path = "server_support/mod.rs"]
mod server_support;
#[path = "sharing_support/project.rs"]
mod support;

use account_cloud::{AccountCloud, EMAIL};
use desktop::{Desktop, linked_server};
use orchester_core::time::Timestamp;
use orchester_store::AcceptedInvite;
use orchester_workspace::sharing::{CloudJoined, cloud::waits};
use server_support::TestServer;

const MINUTE_MS: i64 = 60 * 1000;

/// A desktop signed in to `cloud` that accepted an invite to a server with join approval on, its join waiting; the
/// server and the invite's id.
fn waiting(cloud: &AccountCloud) -> (Desktop, TestServer, String) {
  let (server, audience) = linked_server(cloud);
  cloud.name_server(&audience, &server.url(), None);
  // The account joins from this computer once and signs out: the server asks for approval of it from now on.
  let first = server.email_invite(EMAIL);
  cloud.invite(&first, None);
  let desktop = Desktop::signed_in(cloud);
  let listed = desktop.block_on(desktop.cloud.invites()).expect("invites").remove(0);
  desktop.block_on(desktop.cloud.accept(&listed)).expect("joined");
  let entry = desktop.servers.entries().expect("entries").remove(0);
  desktop.block_on(desktop.servers.sign_out(&entry)).expect("signed out");

  server.set_join_approval(true);
  let second = server.email_invite(EMAIL);
  cloud.invite(&second, None);
  let listed = desktop.block_on(desktop.cloud.invites()).expect("invites").remove(0);
  let accepted = desktop.block_on(desktop.cloud.accept(&listed)).expect("accepted");
  assert!(matches!(accepted.joined, CloudJoined::Waiting(_)), "{accepted:?}");
  (desktop, server, second)
}

/// What is kept of the one invite accepted here.
fn kept(desktop: &Desktop) -> AcceptedInvite {
  desktop
    .block_on(desktop.cloud.accepted_invites())
    .expect("kept")
    .remove(0)
}

/// Checks at `now`; how many invites it asked for.
fn check(desktop: &Desktop, now: Timestamp) -> usize {
  desktop
    .block_on(desktop.cloud.check_accepted(now))
    .expect("checked")
    .len()
}

fn after(at: Timestamp, minutes: i64) -> Timestamp {
  Timestamp::from_unix_millis(at.unix_millis() + minutes * MINUTE_MS)
}

#[test]
fn cloud_asks_a_waiting_join_at_most_ten_times() {
  let cloud = AccountCloud::start();
  let (desktop, _server, invite) = waiting(&cloud);
  for _ in 0..12 {
    // Refused once it asked as often as it does; asked until then.
    let _refused_at_the_end = desktop.block_on(desktop.cloud.finish_accept(kept(&desktop).invite));
  }
  assert_eq!(cloud.tokens_for(&invite), 10, "ten asks, each with its own token");
  let refused = desktop.block_on(desktop.cloud.finish_accept(kept(&desktop).invite));
  assert!(refused.is_err(), "no eleventh ask");
  assert_eq!(cloud.tokens_for(&invite), 10);
}

#[test]
fn cloud_asks_a_waiting_join_again_after_2_4_and_8_minutes_until_its_approval_expires() {
  let cloud = AccountCloud::start();
  let (desktop, _server, invite) = waiting(&cloud);
  let first = kept(&desktop);
  let accepted = first.accepted_at;
  assert_eq!(
    first.next_check_at,
    Some(after(accepted, 2)),
    "asked again on its own in 2 minutes"
  );
  assert_eq!(check(&desktop, accepted), 0, "not yet");
  let mut due = after(accepted, 2);
  for (asks, wait) in [(2, Some(4)), (3, Some(8)), (4, None)] {
    assert_eq!(check(&desktop, after(due, -1)), 0, "not before it is due");
    assert_eq!(check(&desktop, due), 1, "due");
    assert_eq!(cloud.tokens_for(&invite), asks);
    let next = kept(&desktop).next_check_at;
    assert_eq!(
      next,
      wait.map(|minutes| after(due, minutes)),
      "then {wait:?} minutes later"
    );
    due = next.unwrap_or(due);
  }
  let stopped = kept(&desktop);
  assert_eq!(stopped.next_check_at, None);
  assert!(!waits(&stopped, due), "the card asks the person to try again");
  assert_eq!(check(&desktop, after(due, 60)), 0, "stopped");
  assert_eq!(cloud.tokens_for(&invite), 4);

  // The person tries again: asked on its own again, but not once its approval expired.
  desktop
    .block_on(desktop.cloud.finish_accept(stopped.invite))
    .expect("waiting");
  let again = kept(&desktop);
  assert!(
    again.next_check_at.is_some(),
    "asked again on its own after the person asked"
  );
  let expired = after(again.approval_expires_at.expect("the approval's expiry"), 1);
  assert_eq!(check(&desktop, expired), 0, "not after its approval expired");
  assert!(!waits(&kept(&desktop), expired));
  assert_eq!(cloud.tokens_for(&invite), 5);
}
