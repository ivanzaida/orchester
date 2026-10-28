//! The sign-in gate in the window (the person's decision, 2026-10-16; layer *Sign-in gate* `ASl21`): on a first run
//! only the gate shows, and nothing behind it is reachable; signing in opens the window with the tabs it had; the
//! refusals in place; signed in, the app stays open while the cloud cannot be reached and says so in the status bar;
//! each reason the cloud gives brings the gate back with its notice and the email filled in; *Sign out and lock* in
//! Settings › Orchester locks; synthetic input never signs in nor changes the cloud's address; the card and
//! *Advanced* are forms, which Tab goes through and Enter submits ([`keyboard`]); and an account with two-factor
//! authentication finishes its sign-in with a code ([`two_factor`]). Another account on the same computer, with a team
//! server, is checked by the end-to-end tests of the private platform repository (`crates/e2e`, `gate_accounts`).
//!
//! `cargo test -p orchester-app --test gate gate_`

mod came_back;
mod keyboard;
mod states;
#[path = "../support/mod.rs"]
mod support;
mod two_factor;

use lurq::app::events::with_synthetic_input;
use orchester_api::client::ServerAddress;
use orchester_app::routes::{OrchesterView, Route};
use orchester_workspace::sharing::cloud::BUILD_ADDRESS;
use support::{
  Data, Window,
  signed_in::account_cloud::{EMAIL, PASSWORD},
};

/// What only the window of a signed-in session shows: the shell's panel, the sidebar, the tabs and the app menu.
const BEHIND_THE_GATE: [&str; 4] = ["content", "sidebar", "tab-strip", "app-menu"];

/// The window of a new data folder with Kontur in the list, not signed in.
fn signed_out() -> Window {
  let data = Data::new();
  let launch = data.launch_signed_out();
  data.add_kontur(launch.workspace().expect("the session started"));
  let window = Window::of(data, launch);
  window.launch.services().expect("services").links.record_instead();
  window
}

/// The window of a new data folder with Kontur in the list, signed in to the stand-in cloud.
fn signed_in() -> Window {
  let window = Window::open();
  window.launch.services().expect("services").links.record_instead();
  window
}

/// Lays the window out until `id` shows.
fn shows(window: &mut Window, id: &str) {
  window.until(id, |window| window.tree.get_element_by_id(id).is_some());
}

/// Lays the window out until `id` is gone.
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

/// The gate uses the stand-in cloud, as *Advanced · cloud address* sets it.
fn use_stand_in(window: &mut Window) {
  let services = window.launch.services().expect("services").clone();
  let address = support::signed_in::cloud_address(&window.data.cloud);
  window
    .data
    .runtime
    .block_on(services.cloud.set_address(Some(address)))
    .expect("a test cloud");
  window.until("the gate on the test cloud", |window| {
    window
      .texts("gate-footer-cloud")
      .is_some_and(|texts| texts.join("").ends_with("· test cloud"))
  });
}

/// Types `email` and `password` and presses *Sign in* with the pointer, as a person.
fn sign_in(window: &mut Window, (email, password): (&str, &str)) {
  shows(window, "gate-email");
  window.set("gate-email", email);
  window.set("gate-password", password);
  window.press("gate-sign-in");
}

/// The host of the build's own cloud.
fn default_host() -> String {
  ServerAddress::parse(BUILD_ADDRESS)
    .expect("the build's cloud")
    .authority()
}

#[test]
fn gate_shows_on_a_first_run_and_nothing_behind_it_is_reachable() {
  let mut window = signed_out();
  shows(&mut window, "screen-sign-in-gate");
  for id in BEHIND_THE_GATE {
    assert!(!has(&mut window, id), "{id} is behind the gate");
  }
  assert!(has(&mut window, "title-bar-brand"), "the title bar has the mark alone");
  assert_eq!(window.text("gate-eyebrow"), "WELCOME TO ORCHESTER");
  assert_eq!(window.text("gate-title"), "Sign in to start");
  assert!(!has(&mut window, "gate-ended"), "a first run names no reason");
  assert_eq!(value(&mut window, "gate-email"), "");
  assert_eq!(
    window.text("gate-footer-note"),
    "Signing in needs the internet once. After that, Orchester works offline and checks your sign-in when it can \
     reach Orchester Cloud."
  );
  assert_eq!(window.text("gate-footer-cloud"), default_host());
  assert_eq!(window.text("orchester-advanced-meta"), "default");
  assert!(has(&mut window, "status-version"));

  let tabs = window.navigation().tabs().tabs().len();
  window.navigation().go(Route::Settings);
  window.pass();
  assert!(!has(&mut window, "screen-settings") && has(&mut window, "screen-sign-in-gate"));
  for (key, code, ctrl) in [("t", "KeyT", true), ("w", "KeyW", true)] {
    window
      .tree
      .key_down_with_meta(key.into(), code.into(), false, ctrl, false, false);
    window
      .tree
      .key_up_with_meta(key.into(), code.into(), false, ctrl, false, false);
    window.pass();
  }
  assert_eq!(
    window.navigation().tabs().tabs().len(),
    tabs,
    "no shortcut opens or closes a tab"
  );

  window.tree.resize(960, 900);
  window.pass();
  assert_eq!(
    window.text("gate-footer-note"),
    "Signing in needs the internet once. After that, Orchester works offline."
  );
  assert!(!has(&mut window, "status-version"), "the version gives way at 960");
}

#[test]
fn gate_signing_in_opens_the_window_with_its_tabs() {
  let mut window = signed_in();
  let navigation = window.navigation();
  navigation.go(Route::Settings);
  navigation.open_tab();
  navigation.go(Route::Orchester(OrchesterView::default()));
  shows(&mut window, "content");
  let before = window.navigation().tabs();
  let services = window.launch.services().expect("services").clone();
  window
    .data
    .runtime
    .block_on(services.cloud.sign_out())
    .expect("signed out");
  shows(&mut window, "gate-email");
  for id in BEHIND_THE_GATE {
    assert!(!has(&mut window, id), "{id} is behind the gate");
  }
  assert_eq!(value(&mut window, "gate-email"), EMAIL, "the email stays filled in");
  assert_eq!(value(&mut window, "gate-password"), "", "the password is empty");

  window.set("gate-password", PASSWORD);
  window.press("gate-sign-in");
  shows(&mut window, "content");
  assert!(services.cloud.is_signed_in());
  assert_eq!(
    window.navigation().tabs(),
    before,
    "the tabs it had, the same one active"
  );
  assert!(!has(&mut window, "screen-sign-in-gate"));
}

#[test]
fn gate_synthetic_input_cannot_sign_in() {
  let mut window = signed_out();
  use_stand_in(&mut window);
  shows(&mut window, "gate-email");
  window.set("gate-email", EMAIL);
  window.set("gate-password", PASSWORD);
  with_synthetic_input(|| window.press("gate-sign-in"));
  shows(&mut window, "gate-sign-in-person-only");
  assert!(
    window
      .text("gate-sign-in-person-only")
      .contains("Only a person does this here"),
    "the app's person-only reason"
  );
  let services = window.launch.services().expect("services").clone();
  assert!(!services.cloud.is_signed_in());
  assert!(has(&mut window, "screen-sign-in-gate"));
}

/// Scrolls the gate until `id` shows, as a person scrolls to it: the open *Advanced* section ends below a 900 high
/// window.
fn scroll_to(window: &mut Window, id: &str) {
  window
    .tree
    .get_element_by_id_mut(id)
    .unwrap_or_else(|| panic!("{id} is laid out"))
    .scroll_into_view();
  window.pass();
}

/// [`scroll_to`] `id`, then press it with the pointer.
fn press_shown(window: &mut Window, id: &str) {
  scroll_to(window, id);
  window.press(id);
}

#[test]
fn gate_advanced_refuses_synthetic_input() {
  let mut window = signed_out();
  let services = window.launch.services().expect("services").clone();
  shows(&mut window, "orchester-advanced");
  window.press("orchester-advanced");
  shows(&mut window, "orchester-advanced-address");
  window.set("orchester-advanced-address", "http://localhost:9");
  scroll_to(&mut window, "orchester-advanced-use");
  with_synthetic_input(|| window.press("orchester-advanced-use"));
  shows(&mut window, "orchester-advanced-use-person-only");
  assert!(
    services.cloud.address().expect("address").default,
    "the build's cloud stays"
  );

  use_stand_in(&mut window);
  let stand_in = services.cloud.address().expect("address");
  shows(&mut window, "orchester-advanced-reset");
  scroll_to(&mut window, "orchester-advanced-reset");
  with_synthetic_input(|| window.press("orchester-advanced-reset"));
  shows(&mut window, "orchester-advanced-reset-person-only");
  assert_eq!(
    services.cloud.address().expect("address"),
    stand_in,
    "the test cloud stays"
  );
  assert!(has(&mut window, "screen-sign-in-gate"));
}

/// Opens *Advanced*, types `web` and `api` into its fields and presses *Use this address*.
fn use_addresses(window: &mut Window, (web, api): (&str, &str)) {
  shows(window, "orchester-advanced");
  press_shown(window, "orchester-advanced");
  shows(window, "orchester-advanced-api");
  window.set("orchester-advanced-address", web);
  window.set("orchester-advanced-api", api);
  press_shown(window, "orchester-advanced-use");
}

#[test]
fn gate_advanced_takes_an_api_address_or_none() {
  let mut window = signed_out();
  let services = window.launch.services().expect("services").clone();
  shows(&mut window, "orchester-advanced");
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
  scroll_to(&mut window, "gate-sign-in");
  sign_in(&mut window, (EMAIL, PASSWORD));
  shows(&mut window, "content");
  let account = services.cloud.account().expect("account").expect("signed in");
  assert_eq!(account.cloud_api_address, Some(stand_in));
}
