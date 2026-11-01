//! A team server the account joined in the browser (`docs/architecture/server.md`, Invites › *What the desktop sees
//! afterwards*), in Settings › Orchester: Orchester Cloud lists it while this computer has no entry of it, so its row
//! says *Joined · this computer isn't connected* with *Connect*, which joins it with the account as a new computer
//! does (no invite) and gives this computer its own sign-in; with join approval on, the join waits for an admin. Its
//! project is then on the server's page, and *Link a checkout…* opens the invite landing. An invite accepted in the
//! browser whose receipt has not come back stays in the invites, and its *Accept* joins as before.
//!
//! `cargo test -p orchester-app --test join_team join_team_browser_`

use lurq::app::events::with_synthetic_input;
use orchester_api::test_support::team_server::{NAME, TeamServer, USERNAME};
use orchester_app::routes::{OrchesterView, Route};

use super::{Data, INVITE, TEAM, Window, has, joined, shows, window_of};

fn row() -> String {
  format!("orchester-joined-{TEAM}")
}

fn connect() -> String {
  format!("{}-connect", row())
}

/// Settings › Orchester, signed in, with the stand-in cloud listing `server` as the account's and an invite to it.
fn settings(server: &TeamServer) -> Window {
  let mut window = window_of(Data::new(), server, true);
  window.navigation().go(Route::Orchester(OrchesterView::default()));
  shows(&mut window, "orchester-servers");
  window
}

#[test]
fn join_team_browser_joined_server_shows_as_not_connected_and_connect_joins_it() {
  let server = TeamServer::start();
  server.share(&[("Billing", "billing", None)]);
  let mut window = settings(&server);
  shows(&mut window, &connect());
  assert_eq!(
    window.text(&format!("{}-label", row())),
    server.url().trim_start_matches("http://")
  );
  assert_eq!(
    window.text(&format!("{}-hint", row())),
    "Joined · this computer isn't connected"
  );
  assert!(!has(&mut window, "orchester-no-servers"), "the server is listed");

  window.press(&connect());
  shows(&mut window, "orchester-joined-answer-joined");
  assert_eq!(
    window.text("orchester-joined-answer-joined-title"),
    format!("Connected to {NAME} as {USERNAME}")
  );
  assert_eq!(server.cloud_joins(), 1, "one join, with the account and no invite");
  assert_eq!(window.data.cloud.tokens_for(INVITE), 0, "the invite is not used");
  assert_eq!(joined(&window), [NAME]);
  window.until("the server's own row in place of Connect", |window| {
    window.tree.get_element_by_id(&row()).is_none() && window.tree.get_element_by_id(&connect()).is_none()
  });

  window.press("orchester-joined-answer-open");
  shows(&mut window, "orchester-server-project-billing-link");
  window.press("orchester-server-project-billing-link");
  assert!(
    matches!(window.navigation().route_now(), Route::Invite(_)),
    "Link a checkout… opens the invite landing, then the join review"
  );
}

#[test]
fn join_team_browser_joined_connect_waits_for_an_admin_with_approval_on() {
  let server = TeamServer::start();
  server.hold_for_approval();
  let mut window = settings(&server);
  shows(&mut window, &connect());
  window.press(&connect());

  shows(&mut window, "orchester-joined-answer-waiting");
  assert_eq!(
    window.text("orchester-joined-answer-waiting-title"),
    "Waiting for approval"
  );
  assert!(
    window
      .text("orchester-joined-answer-waiting-body")
      .starts_with(&format!("{NAME} asks an admin to approve new computers"))
  );
  assert!(joined(&window).is_empty(), "nothing is joined before the approval");
  assert!(has(&mut window, &connect()), "the row stays");
}

#[test]
fn join_team_browser_joined_only_a_person_connects() {
  let server = TeamServer::start();
  let mut window = settings(&server);
  shows(&mut window, &connect());
  with_synthetic_input(|| window.press(&connect()));

  shows(&mut window, &format!("{}-person-only", connect()));
  assert_eq!(server.cloud_joins(), 0);
  assert!(joined(&window).is_empty());
}

#[test]
fn join_team_browser_accepted_invite_without_receipt_stays_acceptable() {
  let server = TeamServer::start();
  let mut window = window_of(Data::new(), &server, true);
  window.data.cloud.accepted_in_browser(INVITE, 1_000);
  window.navigation().go(Route::Orchester(OrchesterView::default()));
  let accept = format!("orchester-invite-{INVITE}-accept");
  shows(&mut window, &accept);

  window.press(&accept);
  window.until("the invite's server joined", |window| joined(window) == [NAME]);
  assert_eq!(window.data.cloud.tokens_for(INVITE), 1);
}
