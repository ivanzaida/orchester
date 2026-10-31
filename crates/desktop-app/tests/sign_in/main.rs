//! The Orchester Cloud sign-in in Settings › Orchester Cloud (the person's decision, 2026-10-28; board *Signed out ·
//! redesign* `uBND2`): nothing locks without it, the refusals say so in place, each reason the cloud gives ends a
//! sign-in with its notice and the email filled in, synthetic input never signs in nor changes the cloud's address, the
//! fields are a form that Tab goes through and Enter submits, and an account with two-factor authentication finishes
//! its sign-in with a code ([`two_factor`]).
//!
//! `cargo test -p orchester-app --test sign_in sign_in_`

#[path = "../support/mod.rs"]
mod support;
mod two_factor;

use std::time::Duration;

use lurq::app::events::with_synthetic_input;
use orchester_app::routes::{OrchesterView, Route};
use support::{
  Data, Window,
  signed_in::account_cloud::{EMAIL, PASSWORD, PAUSED, UNCONFIRMED},
};

/// The window of a new data folder at Settings › Orchester Cloud, with Kontur in the list and nobody signed in.
fn signed_out() -> Window {
  let data = Data::new();
  let launch = data.launch();
  data.add_kontur(launch.workspace().expect("the session started"));
  let mut window = Window::of(data, launch);
  window.launch.services().expect("services").links.record_instead();
  window.navigation().go(Route::Orchester(OrchesterView::default()));
  shows(&mut window, "sign-in-email");
  window
}

/// [`signed_out`], signed in to the stand-in cloud.
fn signed_in() -> Window {
  let mut window = signed_out();
  use_stand_in(&mut window);
  sign_in(&mut window, (EMAIL, PASSWORD));
  shows(&mut window, ACCOUNT_CARD);
  window
}

/// What only a signed-in column shows.
const ACCOUNT_CARD: &str = "orchester-cloud-header";

fn shows(window: &mut Window, id: &str) {
  window.until(id, |window| window.tree.get_element_by_id(id).is_some());
}

fn gone(window: &mut Window, id: &str) {
  window.until(&format!("{id} gone"), |window| {
    window.tree.get_element_by_id(id).is_none()
  });
}

fn has(window: &mut Window, id: &str) -> bool {
  window.tree.get_element_by_id(id).is_some()
}

/// The value of the text input `id`.
fn value(window: &mut Window, id: &str) -> String {
  window
    .tree
    .get_element_by_id_mut(id)
    .and_then(|element| element.as_text_input())
    .unwrap_or_else(|| panic!("{id} is a text input"))
    .value()
}

/// The stand-in is the cloud from now on, as *Advanced* sets it.
fn use_stand_in(window: &mut Window) {
  let services = window.launch.services().expect("services").clone();
  let address = support::signed_in::cloud_address(&window.data.cloud);
  window
    .data
    .runtime
    .block_on(services.cloud.set_address(Some(address)))
    .expect("a test cloud");
  window.pass();
}

/// Types `email` and `password` and presses *Sign in* with the pointer, as a person.
fn sign_in(window: &mut Window, (email, password): (&str, &str)) {
  shows(window, "sign-in-email");
  window.set("sign-in-email", email);
  window.set("sign-in-password", password);
  window.press("sign-in-submit");
}

/// Presses `key` (its key code `code`) on the keyboard, as a person, and lays the window out.
fn key(window: &mut Window, (key, code): (&str, &str), shift: bool) {
  window.tree.key_down(key.into(), code.into(), shift, false, false);
  window.tree.key_up(key.into(), code.into(), shift, false, false);
  window.pass();
}

fn tab(window: &mut Window) {
  key(window, ("Tab", "Tab"), false);
}

fn enter(window: &mut Window) {
  key(window, ("Enter", "Enter"), false);
}

fn focused(window: &Window) -> Option<String> {
  window
    .tree
    .focused_element()
    .and_then(|element| element.id().map(str::to_owned))
}

fn focus_on(window: &mut Window, id: &str) {
  window.until(&format!("the focus on {id}"), |window| {
    focused(window).as_deref() == Some(id)
  });
}

/// Asks the cloud whether the sign-in holds, as the app does when it can reach it.
fn check(window: &mut Window) {
  let services = window.launch.services().expect("services").clone();
  let checked = window.data.runtime.block_on(services.cloud.check_sign_in());
  tracing::debug!(?checked, "checked the sign-in");
  window.pass();
}

#[test]
fn sign_in_offers_itself_in_settings_on_a_first_run() {
  let mut window = signed_out();
  assert_eq!(window.text("sign-in-title"), "Not signed in");
  assert_eq!(
    window.text("sign-in-lead"),
    "Your projects on this computer work without an account. Sign in to join teams and accept invites."
  );
  assert!(!has(&mut window, "sign-in-ended"), "a first run names no reason");
  assert_eq!(value(&mut window, "sign-in-email"), "");
  assert_eq!(window.text("orchester-invites-sign-in-label"), "Sign in to see invites");
  assert!(has(&mut window, "orchester-servers"), "team servers stay");
  assert!(has(&mut window, "orchester-advanced"), "and Advanced");
  assert!(
    !has(&mut window, "orchester-cloud-computer"),
    "This computer goes while signed out"
  );
  assert!(has(&mut window, "sidebar"), "nothing locks: the window is the same");
  assert!(has(&mut window, "tab-strip"));
}

#[test]
fn sign_in_shows_the_account_with_no_navigation() {
  let mut window = signed_out();
  use_stand_in(&mut window);
  let route = window.navigation().route_now();
  sign_in(&mut window, (EMAIL, PASSWORD));
  shows(&mut window, ACCOUNT_CARD);
  assert_eq!(window.navigation().route_now(), route, "the same page");
  assert!(!has(&mut window, "sign-in-card"));
}

#[test]
fn sign_in_says_a_wrong_email_or_password_in_place() {
  let mut window = signed_out();
  use_stand_in(&mut window);
  sign_in(&mut window, (EMAIL, "not the password"));
  shows(&mut window, "sign-in-wrong");
  assert_eq!(
    window.text("sign-in-wrong"),
    "The email or password is not right. Check both, or reset your password."
  );
  assert_eq!(value(&mut window, "sign-in-email"), EMAIL, "the email stays");
  assert_eq!(
    value(&mut window, "sign-in-password"),
    "",
    "the password is typed again"
  );
}

#[test]
fn sign_in_asks_to_confirm_the_email_first_and_opens_the_clouds_pages() {
  let mut window = signed_out();
  use_stand_in(&mut window);
  sign_in(&mut window, (UNCONFIRMED, "any password at all"));
  shows(&mut window, "sign-in-unconfirmed");
  assert_eq!(window.text("sign-in-unconfirmed-title"), "Confirm your email first");
  window.press("sign-in-open-cloud");
  window.press("sign-in-create-account");
  window.press("sign-in-forgot");
  let cloud = window.data.cloud.url();
  assert_eq!(
    window.launch.services().expect("services").links.recorded(),
    vec![
      format!("{cloud}/signin"),
      format!("{cloud}/signup"),
      format!("{cloud}/forgot")
    ]
  );
}

#[test]
fn sign_in_pauses_an_email_with_too_many_attempts() {
  let mut window = signed_out();
  use_stand_in(&mut window);
  sign_in(&mut window, (PAUSED, "any password at all"));
  shows(&mut window, "sign-in-paused");
  assert_eq!(window.text("sign-in-paused-title"), "Too many attempts");
  window.press("sign-in-submit");
  assert!(
    !has(&mut window, "sign-in-checking"),
    "Sign in waits until the pause is over"
  );
  enter(&mut window);
  assert!(!has(&mut window, "sign-in-checking"), "and so does Enter");
  window.set("sign-in-email", EMAIL);
  window.set("sign-in-password", PASSWORD);
  window.press("sign-in-submit");
  shows(&mut window, ACCOUNT_CARD);
}

#[test]
fn sign_in_says_the_internet_is_needed() {
  let mut window = signed_out();
  use_stand_in(&mut window);
  window.data.cloud.set_down(true);
  sign_in(&mut window, (EMAIL, PASSWORD));
  shows(&mut window, "sign-in-unreachable");
  let host = format!("localhost:{}", window.data.cloud.address.port());
  let note = window.text("sign-in-unreachable-note");
  assert!(note.starts_with(&format!("{host} didn't answer · ")), "{note}");
  assert!(has(&mut window, "sidebar"), "nothing else is held up");
}

#[test]
fn sign_in_says_it_is_signing_in() {
  let mut window = signed_out();
  use_stand_in(&mut window);
  window.data.cloud.slow_sign_in(Duration::from_secs(2));
  sign_in(&mut window, (EMAIL, PASSWORD));
  shows(&mut window, "sign-in-checking");
  let words = format!("Checking with localhost:{}…", window.data.cloud.address.port());
  window.until("the stand-in's host", |window| {
    window
      .texts("sign-in-checking")
      .is_some_and(|texts| texts.join("") == words)
  });
  assert_eq!(window.text("sign-in-submit"), "Signing in…");
  assert!(
    has(&mut window, "orchester-advanced"),
    "the rest of the page stays usable"
  );
  shows(&mut window, ACCOUNT_CARD);
}

#[test]
fn sign_in_synthetic_input_cannot_sign_in() {
  let mut window = signed_out();
  use_stand_in(&mut window);
  window.set("sign-in-email", EMAIL);
  window.set("sign-in-password", PASSWORD);
  with_synthetic_input(|| window.press("sign-in-submit"));
  shows(&mut window, "sign-in-submit-person-only");
  assert!(
    window
      .text("sign-in-submit-person-only")
      .contains("Only a person does this here"),
    "the app's person-only reason"
  );
  let services = window.launch.services().expect("services").clone();
  assert!(!services.cloud.is_signed_in());
  with_synthetic_input(|| enter(&mut window));
  assert!(!has(&mut window, "sign-in-checking"), "Enter neither");
  assert!(!services.cloud.is_signed_in());
}

#[test]
fn sign_in_advanced_refuses_synthetic_input() {
  let mut window = signed_out();
  let services = window.launch.services().expect("services").clone();
  window.press("orchester-advanced");
  shows(&mut window, "orchester-advanced-address");
  window.set("orchester-advanced-address", "http://localhost:9");
  window
    .tree
    .get_element_by_id_mut("orchester-advanced-use")
    .expect("laid out")
    .scroll_into_view();
  window.pass();
  with_synthetic_input(|| window.press("orchester-advanced-use"));
  shows(&mut window, "orchester-advanced-use-person-only");
  assert!(
    services.cloud.address().expect("address").default,
    "the build's cloud stays"
  );
}

/// Brings `id` into the column's view and presses it.
fn press_shown(window: &mut Window, id: &str) {
  window
    .tree
    .get_element_by_id_mut(id)
    .expect("laid out")
    .scroll_into_view();
  window.pass();
  window.press(id);
}

/// Opens *Advanced* and uses the cloud's address `web` with the API address `api`.
fn use_addresses(window: &mut Window, (web, api): (&str, &str)) {
  shows(window, "orchester-advanced");
  press_shown(window, "orchester-advanced");
  shows(window, "orchester-advanced-api");
  window.set("orchester-advanced-address", web);
  window.set("orchester-advanced-api", api);
  press_shown(window, "orchester-advanced-use");
}

#[test]
fn sign_in_advanced_takes_an_api_address_or_none() {
  let mut window = signed_out();
  let services = window.launch.services().expect("services").clone();
  window.press("orchester-advanced");
  shows(&mut window, "orchester-advanced-api");
  let build = services.cloud.address().expect("address").address;
  assert_eq!(value(&mut window, "orchester-advanced-address"), build.web().as_str());
  let build_api = build.api().map(|api| api.as_str().to_owned()).unwrap_or_default();
  assert_eq!(
    value(&mut window, "orchester-advanced-api"),
    build_api,
    "the build's API address"
  );
  press_shown(&mut window, "orchester-advanced");
  gone(&mut window, "orchester-advanced-api");

  let stand_in = window.data.cloud.url();
  use_addresses(&mut window, (&stand_in, "ftp://api.test"));
  shows(&mut window, "orchester-advanced-error");
  assert!(
    window
      .text("orchester-advanced-error")
      .contains("That isn't an API address.")
  );
  assert!(services.cloud.address().expect("address").default, "nothing changed");
  press_shown(&mut window, "orchester-advanced");
  gone(&mut window, "orchester-advanced-api");

  // Without an API address, the API is at the old paths on the cloud's address.
  use_addresses(&mut window, (&stand_in, ""));
  gone(&mut window, "orchester-advanced-api");
  let set = services.cloud.address().expect("address").address;
  assert_eq!(set.api_base(), format!("{stand_in}/api/v1"));

  use_addresses(&mut window, (&stand_in, &stand_in));
  gone(&mut window, "orchester-advanced-api");
  let set = services.cloud.address().expect("address").address;
  assert_eq!(set.api_base(), format!("{stand_in}/v1"));
  press_shown(&mut window, "sign-in-submit");
  sign_in(&mut window, (EMAIL, PASSWORD));
  shows(&mut window, ACCOUNT_CARD);
  let account = services.cloud.account().expect("account").expect("signed in");
  assert_eq!(account.cloud_api_address, Some(stand_in));
}

#[test]
fn sign_in_enter_does_nothing_while_signing_in() {
  let mut window = signed_out();
  use_stand_in(&mut window);
  window.data.cloud.slow_sign_in(Duration::from_secs(2));
  focus_on(&mut window, "sign-in-email");
  window.set("sign-in-email", EMAIL);
  window.set("sign-in-password", PASSWORD);
  enter(&mut window);
  shows(&mut window, "sign-in-checking");
  assert_eq!(
    focused(&window).as_deref(),
    Some("sign-in-email"),
    "the faded field keeps the focus"
  );
  // Another Enter with a wrong password would replace the sign-in under way and be refused.
  window.set("sign-in-password", "not the password");
  enter(&mut window);
  shows(&mut window, ACCOUNT_CARD);
  assert!(!has(&mut window, "sign-in-wrong"));
}

#[test]
fn sign_in_enter_does_nothing_while_paused() {
  let mut window = signed_out();
  use_stand_in(&mut window);
  sign_in(&mut window, (PAUSED, "any password at all"));
  shows(&mut window, "sign-in-paused");
  window.press("sign-in-email");
  focus_on(&mut window, "sign-in-email");
  enter(&mut window);
  assert!(
    !has(&mut window, "sign-in-checking"),
    "Enter waits until the pause is over"
  );
  assert!(has(&mut window, "sign-in-paused"));
}

#[test]
fn sign_in_tab_goes_through_the_fields_in_order_and_enter_signs_in() {
  let mut window = signed_out();
  use_stand_in(&mut window);
  focus_on(&mut window, "sign-in-email");
  let mut stops = Vec::new();
  for _ in 0..4 {
    tab(&mut window);
    stops.push(focused(&window).unwrap_or_default());
  }
  assert_eq!(
    stops,
    [
      "sign-in-password",
      "sign-in-forgot",
      "sign-in-submit",
      "sign-in-create-account"
    ]
  );
  window.press("sign-in-email");
  focus_on(&mut window, "sign-in-email");
  enter(&mut window);
  window.set("sign-in-email", EMAIL);
  enter(&mut window);
  focus_on(&mut window, "sign-in-password");
  assert!(
    !has(&mut window, "sign-in-checking"),
    "an empty field takes the focus; nothing was sent"
  );
  window.set("sign-in-password", PASSWORD);
  enter(&mut window);
  shows(&mut window, ACCOUNT_CARD);
}

#[test]
fn sign_in_ended_says_each_reason_the_cloud_gives() {
  for (reason, title) in [
    (Some("revoked"), "This computer was signed out"),
    (Some("password_reset"), "Your password was changed"),
    (Some("account_deleted"), "Your account was deleted"),
    (Some("signed_out"), "You signed out"),
    (None, "Your sign-in ended"),
  ] {
    let mut window = signed_in();
    window.data.cloud.end_token(reason, 1_792_147_957_042);
    check(&mut window);
    shows(&mut window, "sign-in-notice");
    assert_eq!(window.text("sign-in-notice-title"), title, "{reason:?}");
    assert_eq!(window.text("sign-in-title"), "Sign in again");
    assert_eq!(
      value(&mut window, "sign-in-email"),
      EMAIL,
      "{reason:?}: the email stays"
    );
    focus_on(&mut window, "sign-in-password");
    assert!(has(&mut window, "sidebar"), "{reason:?}: nothing locks");
  }
}

#[test]
fn sign_in_sign_out_asks_in_place_and_nothing_locks() {
  let mut window = signed_in();
  assert_eq!(
    window.text("orchester-cloud-signing-out-text"),
    "Sign out ends this computer's session with Orchester Cloud. Projects on this computer keep working; your \
     account's projects wait until you sign in again, and their runs keep going."
  );
  window.press("orchester-cloud-sign-out");
  shows(&mut window, "orchester-cloud-sign-out-question");
  // Asked in place under the account card (`xQQbr`); the notice's icon glyph is left out.
  assert_eq!(
    window.texts("orchester-cloud-sign-out-question").expect("the question"),
    [
      "Sign out of Orchester Cloud on this computer?",
      "Projects on this computer keep working. Your account's projects wait until you sign in again, and their runs \
       keep going. A team server joined with a code stays; ones joined through this account wait too.",
      "Sign out",
      "Stay signed in",
    ]
  );
  window.press("orchester-cloud-sign-out-stay");
  gone(&mut window, "orchester-cloud-sign-out-question");
  assert!(window.launch.services().expect("services").cloud.is_signed_in());

  window.press("orchester-cloud-sign-out");
  shows(&mut window, "orchester-cloud-sign-out-confirm");
  window.press("orchester-cloud-sign-out-confirm");
  shows(&mut window, "sign-in-notice");
  assert_eq!(window.text("sign-in-notice-title"), "You signed out");
  assert!(
    window.data.cloud.signed_out(),
    "the cloud ended this computer's session"
  );
  assert!(has(&mut window, "sidebar"));
}

#[test]
fn sign_in_offline_nothing_locks_and_the_status_bar_says_so() {
  let mut window = signed_in();
  assert!(!has(&mut window, "status-cloud"), "nothing while the cloud answers");
  window.data.cloud.set_down(true);
  check(&mut window);
  shows(&mut window, "status-cloud");
  assert_eq!(window.text("status-cloud-text"), "Orchester Cloud · offline");
  window.data.cloud.set_down(false);
  check(&mut window);
  gone(&mut window, "status-cloud");
}
