//! The invite landing while accepting its invite needs a sign-in first (board *Signed out · redesign* `uBND2`): with
//! nobody signed in it asks for the sign-in in place (`zoihT`); signed in without a code to an account with two-factor
//! authentication on, it says *Sign in again to accept* (`d1yW2I`), whose *Sign in again* is a person's and leaves the
//! landing where it is, so the sign-in with the code opens on it.
//!
//! `cargo test -p orchester-app --test invite_landing invite_landing_`

mod support;

use lurq::app::events::with_synthetic_input;
use orchester_app::routes::{JoinTarget, OrchesterView, Route};
use orchester_core::id::ServerId;
use support::{
  Data, Window,
  signed_in::{self, account_cloud::EMAIL},
};

const AGAIN: &str = "orchester-landing-sign-in-again";
const SIGN_IN_AGAIN: &str = "orchester-landing-sign-in-again-sign-in-again";

/// The window of a new data folder with the stand-in set as the cloud, nobody signed in.
fn fresh() -> Window {
  let data = Data::new();
  let launch = data.launch();
  let services = launch.services().expect("the session started").clone();
  data
    .runtime
    .block_on(services.cloud.set_address(Some(signed_in::cloud_address(&data.cloud))))
    .expect("a test cloud");
  Window::of(data, launch)
}

/// [`fresh`], signed in before two-factor authentication was turned on, so the session has no code.
fn signed_in_without_a_code() -> Window {
  let mut window = fresh();
  let services = window.launch.services().expect("services").clone();
  signed_in::sign_in(&services, window.data.runtime.handle(), &window.data.cloud);
  window.data.cloud.session_without_two_factor();
  window.pass();
  window
}

/// The landing of an invite to Kontur on a server this computer has not joined.
fn landing() -> Route {
  Route::Invite(JoinTarget::new(ServerId::generate(), "kontur").expect("a slug"))
}

fn shows(window: &mut Window, id: &str) {
  window.until(id, |window| window.tree.get_element_by_id(id).is_some());
}

#[test]
fn invite_landing_signed_out_asks_for_the_sign_in_in_place() {
  let mut window = fresh();
  window.navigation().go(landing());
  shows(&mut window, "sign-in-email");
  assert_eq!(
    window.text("orchester-landing-signed-out-lead"),
    "Sign in to Orchester Cloud to accept this invite. Nothing changes on this computer until you've picked a \
     checkout and read the join review."
  );
  assert!(window.tree.get_element_by_id(AGAIN).is_none());
}

#[test]
fn invite_landing_signed_in_without_a_code_asks_to_sign_in_again() {
  let mut window = signed_in_without_a_code();
  window.navigation().go(landing());
  shows(&mut window, AGAIN);
  assert_eq!(
    window.text("orchester-landing-signed-out-lead"),
    format!("This computer is signed in as {EMAIL}. Accepting this invite needs a sign-in with your two-factor code.")
  );
  assert_eq!(window.text(&format!("{AGAIN}-title")), "Sign in again to accept");
  assert_eq!(
    window.text(&format!("{AGAIN}-body")),
    "This computer signed in before you turned on two-factor. Accepting needs a sign-in with your code."
  );
  assert_eq!(
    window.text(&format!("{AGAIN}-note")),
    "Your projects and runs aren't affected, and the invite waits here."
  );
  assert_eq!(window.text("orchester-landing-project-value"), "kontur");
  assert!(window.tree.get_element_by_id("sign-in-email").is_none());
}

#[test]
fn invite_landing_waits_for_the_two_factor_status_before_saying_anything() {
  let mut window = signed_in_without_a_code();
  window.data.cloud.hold_checks();
  let arrived = window.data.cloud.checks_arrived();
  window.navigation().go(landing());
  window.until("the two-factor status asked for", |window| {
    window.data.cloud.checks_arrived() > arrived
  });
  shows(&mut window, "orchester-landing-loading");
  assert!(window.tree.get_element_by_id("orchester-landing-failed").is_none());
  assert!(window.tree.get_element_by_id(AGAIN).is_none());

  window.data.cloud.let_checks_go();
  shows(&mut window, AGAIN);
}

#[test]
fn invite_landing_sign_in_again_opens_the_sign_in_on_the_landing() {
  let mut window = signed_in_without_a_code();
  let route = landing();
  window.navigation().go(route.clone());
  shows(&mut window, SIGN_IN_AGAIN);

  window.press(SIGN_IN_AGAIN);
  shows(&mut window, "sign-in-email");
  assert_eq!(window.navigation().route_now(), route, "the sign-in opens in place");
  shows(&mut window, "sign-in-notice");
  assert!(window.tree.get_element_by_id(AGAIN).is_none());
  let services = window.launch.services().expect("services").clone();
  assert!(!services.cloud.is_signed_in());
}

#[test]
fn invite_landing_sign_in_again_is_a_persons() {
  let mut window = signed_in_without_a_code();
  window.navigation().go(landing());
  shows(&mut window, SIGN_IN_AGAIN);

  with_synthetic_input(|| window.press(SIGN_IN_AGAIN));
  shows(&mut window, &format!("{SIGN_IN_AGAIN}-person-only"));
  let services = window.launch.services().expect("services").clone();
  assert!(services.cloud.is_signed_in(), "still signed in");
}

#[test]
fn invite_landing_not_now_goes_back_to_orchester_cloud() {
  let mut window = signed_in_without_a_code();
  window.navigation().go(landing());
  shows(&mut window, &format!("{AGAIN}-not-now"));

  window.press(&format!("{AGAIN}-not-now"));
  window.until("Settings › Orchester Cloud", |window| {
    window.navigation().route_now() == Route::Orchester(OrchesterView::default())
  });
  let services = window.launch.services().expect("services").clone();
  assert!(services.cloud.is_signed_in(), "nothing signed out");
}

#[test]
fn invite_landing_signed_in_with_a_code_does_not_ask_again() {
  let mut window = fresh();
  let services = window.launch.services().expect("services").clone();
  signed_in::sign_in(&services, window.data.runtime.handle(), &window.data.cloud);
  window.pass();
  window.navigation().go(landing());
  shows(&mut window, "orchester-landing-failed");
  assert!(window.tree.get_element_by_id(AGAIN).is_none());
  assert!(window.tree.get_element_by_id("sign-in-card").is_none());
}
