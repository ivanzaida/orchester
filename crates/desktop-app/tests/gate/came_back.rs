//! Signed in: the app stays open while the cloud cannot be reached (`ZSB6h`); the gate comes back with the cloud's
//! reason (`obc19`, `ztigw`) and after *Sign out and lock* (`p2VeL`, `mnT1S`).

use std::time::{Duration, Instant};

use orchester_app::{
  routes::{OrchesterView, Route},
  ui::tooltip::TOOLTIP_DELAY,
};

use super::{BEHIND_THE_GATE, gone, has, shows, signed_in, support::Window, value};
use crate::support::signed_in::account_cloud::EMAIL;

/// Rests the pointer on the middle of `id`.
fn rest_on(window: &mut Window, id: &str) {
  let bounds = window
    .tree
    .get_element_by_id_mut(id)
    .and_then(|element| element.bounds())
    .unwrap_or_else(|| panic!("{id} is laid out"));
  window
    .tree
    .mouse_move(bounds.x + bounds.width / 2.0, bounds.y + bounds.height / 2.0);
  window.pass();
  window
    .tree
    .tick_timers_at(Instant::now() + TOOLTIP_DELAY + Duration::from_millis(50));
  window.pass();
}

/// Asks the cloud whether the sign-in holds, as the app does when it can reach it.
fn check(window: &mut Window) {
  let services = window.launch.services().expect("services").clone();
  let checked = window.data.runtime.block_on(services.cloud.check_sign_in());
  tracing::debug!(?checked, "checked the sign-in");
  window.pass();
}

#[test]
fn gate_stays_open_offline_and_the_status_bar_says_so() {
  let mut window = signed_in();
  shows(&mut window, "content");
  assert!(!has(&mut window, "status-cloud"), "nothing while the cloud answers");
  window.data.cloud.set_down(true);
  check(&mut window);
  shows(&mut window, "status-cloud");
  assert_eq!(window.text("status-cloud-text"), "Orchester Cloud · offline");
  assert!(has(&mut window, "content"), "offline-first: nothing locks");
  rest_on(&mut window, "status-cloud");
  shows(&mut window, "status-cloud-card");
  assert_eq!(window.text("status-cloud-card-hint"), "Click for Settings › Orchester.");
  let state = window.text("status-cloud-card-state");
  assert!(
    state.starts_with("Offline since ") && state.ends_with(&format!(" · signed in as {EMAIL}")),
    "{state}"
  );
  window.press("status-cloud");
  shows(&mut window, "orchester-cloud-details");

  window.data.cloud.set_down(false);
  check(&mut window);
  gone(&mut window, "status-cloud");
  assert!(has(&mut window, "content"));
}

#[test]
fn gate_comes_back_with_each_reason_the_cloud_gives() {
  for (reason, title) in [
    (Some("revoked"), "This computer was signed out"),
    (Some("password_reset"), "Your password was changed"),
    (Some("account_deleted"), "Your account was deleted"),
    (Some("signed_out"), "You signed out"),
    (None, "Your sign-in ended"),
  ] {
    let mut window = signed_in();
    shows(&mut window, "content");
    window.data.cloud.end_token(reason, 1_792_147_957_042);
    check(&mut window);
    shows(&mut window, "gate-notice");
    for id in BEHIND_THE_GATE {
      assert!(!has(&mut window, id), "{reason:?}: {id} is behind the gate");
    }
    assert_eq!(window.text("gate-notice-title"), title, "{reason:?}");
    assert_eq!(window.text("gate-title"), "Sign in again");
    assert_eq!(window.text("gate-eyebrow"), "ORCHESTER CLOUD");
    assert_eq!(
      value(&mut window, "gate-email"),
      EMAIL,
      "{reason:?}: the email stays filled in"
    );
    assert_eq!(
      window.text("gate-footer-note"),
      "Only you can sign in, here in this window. Orchester's MCP tools can't sign in for you."
    );
    let rows = window.texts("gate-while-signed-out").expect("what happens meanwhile");
    assert_eq!(
      rows,
      [
        "Your projects and tasks stay on this computer.",
        "Runs that are going keep going. Anything that needs you waits until you're back.",
        "Agents and other MCP clients get “sign in required” until then.",
      ]
    );
  }
}

#[test]
fn gate_password_change_names_the_account() {
  let mut window = signed_in();
  shows(&mut window, "content");
  window.data.cloud.end_token(Some("password_reset"), 1_792_147_957_042);
  check(&mut window);
  shows(&mut window, "gate-notice");
  assert_eq!(
    window.text("gate-notice-body"),
    format!(
      "Changing or resetting the password of {EMAIL} signs out your other computers, this one too. Sign in with the \
       new password."
    )
  );
}

#[test]
fn gate_sign_out_asks_in_place_and_locks() {
  let mut window = signed_in();
  window.navigation().go(Route::Orchester(OrchesterView::default()));
  shows(&mut window, "orchester-cloud-sign-out");
  assert_eq!(
    window.text("orchester-cloud-signing-out-text"),
    "Sign out ends this computer's session with Orchester Cloud and locks Orchester until you sign in again. \
     Projects, tasks and runs on this computer are not touched, and team servers keep this computer's own sign-in."
  );
  window.press("orchester-cloud-sign-out");
  shows(&mut window, "orchester-cloud-sign-out-question");
  // Asked in place under the account card (`Sign out` `Q38L6j`); the notice's icon glyph is left out.
  assert_eq!(
    window.texts("orchester-cloud-sign-out-question").expect("the question"),
    [
      "Sign out of Orchester Cloud on this computer?",
      "Orchester locks when you sign out: this window shows only the sign-in until you sign in again, and its MCP \
       tools answer “sign in required”.",
      "Your projects and tasks stay on this computer, and runs that are going keep going. Team servers keep this \
       computer's own sign-in, so nothing needs joining again.",
      "Sign out and lock",
      "Stay signed in",
    ]
  );
  window.press("orchester-cloud-sign-out-stay");
  gone(&mut window, "orchester-cloud-sign-out-question");
  assert!(window.launch.services().expect("services").cloud.is_signed_in());

  window.press("orchester-cloud-sign-out");
  shows(&mut window, "orchester-cloud-sign-out-confirm");
  window.press("orchester-cloud-sign-out-confirm");
  shows(&mut window, "gate-notice");
  assert_eq!(window.text("gate-notice-title"), "You signed out");
  assert_eq!(
    window.text("gate-notice-body"),
    "Orchester stays locked on this computer until you sign in again."
  );
  assert!(
    window.data.cloud.signed_out(),
    "the cloud ended this computer's session"
  );
  for id in BEHIND_THE_GATE {
    assert!(!has(&mut window, id), "{id} is behind the gate");
  }
}
