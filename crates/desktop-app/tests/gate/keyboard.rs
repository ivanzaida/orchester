//! The gate's card and *Advanced · cloud address* as lurq forms: Tab goes through them in the order they show, Enter in
//! a field does what its button does, with the button's refusals, and the first field to type in takes the focus.

use std::time::Duration;

use lurq::app::events::with_synthetic_input;

use super::{
  gone, has, shows, sign_in, signed_in, signed_out,
  support::{
    Window,
    signed_in::account_cloud::{EMAIL, PASSWORD, PAUSED},
  },
  use_stand_in,
};

/// The card's stops and the closed Advanced line, in the order Tab reaches them.
const CARD_ORDER: [&str; 6] = [
  "gate-email",
  "gate-password",
  "gate-forgot",
  "gate-sign-in",
  "gate-create-account",
  "orchester-advanced",
];

/// Presses `key` (its key code `code`) on the keyboard, as a person, and lays the window out.
fn key(window: &mut Window, (key, code): (&str, &str), shift: bool) {
  window.tree.key_down(key.into(), code.into(), shift, false, false);
  window.tree.key_up(key.into(), code.into(), shift, false, false);
  window.pass();
}

fn tab(window: &mut Window, back: bool) {
  key(window, ("Tab", "Tab"), back);
}

fn enter(window: &mut Window) {
  key(window, ("Enter", "Enter"), false);
}

/// The id of the element that has the focus.
fn focused(window: &Window) -> Option<String> {
  window
    .tree
    .focused_element()
    .and_then(|element| element.id().map(str::to_owned))
}

/// Lays the window out until `id` has the focus.
fn focus_on(window: &mut Window, id: &str) {
  window.until(&format!("the focus on {id}"), |window| {
    focused(window).as_deref() == Some(id)
  });
}

/// The stops Tab (or Shift+Tab, `back`) reaches from the focused element, `count` of them.
fn walk(window: &mut Window, back: bool, count: usize) -> Vec<String> {
  (0..count)
    .map(|_| {
      tab(window, back);
      focused(window).unwrap_or_default()
    })
    .collect()
}

#[test]
fn gate_tab_goes_through_the_card_and_advanced_in_order() {
  let mut window = signed_out();
  focus_on(&mut window, "gate-email");

  let forward = walk(&mut window, false, CARD_ORDER.len());
  let mut expected: Vec<&str> = CARD_ORDER[1..].to_vec();
  expected.push(CARD_ORDER[0]);
  assert_eq!(forward, expected, "Tab, and around to the email again");
  let back = walk(&mut window, true, CARD_ORDER.len());
  let expected: Vec<&str> = CARD_ORDER.iter().rev().copied().collect();
  assert_eq!(
    back, expected,
    "Shift+Tab, around to the Advanced line and back to the email"
  );

  tab(&mut window, true);
  assert_eq!(focused(&window).as_deref(), Some("orchester-advanced"));
  enter(&mut window);
  shows(&mut window, "orchester-advanced-address");
  assert_eq!(
    walk(&mut window, false, 5),
    [
      "orchester-advanced-address",
      "orchester-advanced-api",
      "orchester-advanced-use",
      "orchester-advanced-reset",
      "gate-email"
    ],
    "the open section's fields and buttons after its line"
  );
}

#[test]
fn gate_focus_starts_in_the_first_field_to_type_in() {
  let mut window = signed_out();
  focus_on(&mut window, "gate-email");

  let mut window = signed_in();
  shows(&mut window, "content");
  let services = window.launch.services().expect("services").clone();
  window
    .data
    .runtime
    .block_on(services.cloud.sign_out())
    .expect("signed out");
  shows(&mut window, "gate-email");
  focus_on(&mut window, "gate-password");
}

#[test]
fn gate_enter_signs_in_from_either_field() {
  let mut window = signed_out();
  use_stand_in(&mut window);
  focus_on(&mut window, "gate-email");
  window.set("gate-email", EMAIL);
  window.set("gate-password", "not the password");
  enter(&mut window);
  shows(&mut window, "gate-wrong");

  window.set("gate-password", PASSWORD);
  tab(&mut window, false);
  assert_eq!(focused(&window).as_deref(), Some("gate-password"));
  enter(&mut window);
  shows(&mut window, "content");
  let services = window.launch.services().expect("services").clone();
  assert!(services.cloud.is_signed_in());
}

#[test]
fn gate_enter_with_an_empty_field_moves_to_it() {
  let mut window = signed_out();
  use_stand_in(&mut window);
  focus_on(&mut window, "gate-email");
  enter(&mut window);
  window.set("gate-email", EMAIL);
  enter(&mut window);
  focus_on(&mut window, "gate-password");
  window.press("gate-sign-in");
  window.pass();
  assert!(!has(&mut window, "gate-checking"), "nothing was sent");
  assert!(!has(&mut window, "gate-wrong"), "and nothing refused");
  focus_on(&mut window, "gate-password");
}

#[test]
fn gate_enter_does_nothing_while_signing_in() {
  let mut window = signed_out();
  use_stand_in(&mut window);
  window.data.cloud.slow_sign_in(Duration::from_secs(2));
  focus_on(&mut window, "gate-email");
  window.set("gate-email", EMAIL);
  window.set("gate-password", PASSWORD);
  enter(&mut window);
  shows(&mut window, "gate-checking");
  assert_eq!(
    focused(&window).as_deref(),
    Some("gate-email"),
    "the faded field keeps the focus"
  );
  // Another Enter with a wrong password would replace the sign-in under way and be refused.
  window.set("gate-password", "not the password");
  enter(&mut window);
  shows(&mut window, "content");
  assert!(!has(&mut window, "gate-wrong"));
}

#[test]
fn gate_enter_does_nothing_while_paused() {
  let mut window = signed_out();
  use_stand_in(&mut window);
  sign_in(&mut window, (PAUSED, "any password at all"));
  shows(&mut window, "gate-paused");
  window.press("gate-email");
  focus_on(&mut window, "gate-email");
  enter(&mut window);
  assert!(
    !has(&mut window, "gate-checking"),
    "Enter waits until the pause is over"
  );
  assert!(has(&mut window, "gate-paused"));
}

#[test]
fn gate_synthetic_enter_cannot_sign_in() {
  let mut window = signed_out();
  use_stand_in(&mut window);
  focus_on(&mut window, "gate-email");
  window.set("gate-email", EMAIL);
  window.set("gate-password", PASSWORD);
  with_synthetic_input(|| enter(&mut window));
  shows(&mut window, "gate-sign-in-person-only");
  assert!(!has(&mut window, "gate-checking"));
  let services = window.launch.services().expect("services").clone();
  assert!(!services.cloud.is_signed_in());
}

#[test]
fn gate_enter_in_advanced_uses_the_address() {
  let mut window = signed_out();
  let services = window.launch.services().expect("services").clone();
  shows(&mut window, "orchester-advanced");
  window.press("orchester-advanced");
  shows(&mut window, "orchester-advanced-address");
  let stand_in = window.data.cloud.url();
  window.set("orchester-advanced-address", &stand_in);
  window.press("orchester-advanced-address");
  focus_on(&mut window, "orchester-advanced-address");

  with_synthetic_input(|| enter(&mut window));
  shows(&mut window, "orchester-advanced-use-person-only");
  assert!(
    services.cloud.address().expect("address").default,
    "the build's cloud stays"
  );

  enter(&mut window);
  gone(&mut window, "orchester-advanced-address");
  let address = services.cloud.address().expect("address");
  assert!(!address.default, "the stand-in is the cloud now");
  window.until("the gate on the test cloud", |window| {
    window
      .texts("gate-footer-cloud")
      .is_some_and(|texts| texts.join("").ends_with("· test cloud"))
  });
}

#[test]
fn gate_two_factor_tab_and_enter_go_through_the_code_step() {
  let mut window = signed_out();
  use_stand_in(&mut window);
  window.data.cloud.require_codes();
  focus_on(&mut window, "gate-email");
  window.set("gate-email", EMAIL);
  window.set("gate-password", PASSWORD);
  enter(&mut window);
  shows(&mut window, "gate-code-card");
  focus_on(&mut window, "gate-code");
  assert_eq!(
    walk(&mut window, false, 4),
    ["gate-code-back", "gate-verify", "gate-code-switch", "gate-code"],
    "the field, Back, Verify and the link, and around"
  );

  window.set("gate-code", "000000");
  enter(&mut window);
  shows(&mut window, "gate-code-note");
  let code = window.data.cloud.next_code();
  window.set("gate-code", &code);
  with_synthetic_input(|| enter(&mut window));
  shows(&mut window, "gate-verify-person-only");
  assert!(!has(&mut window, "gate-code-checking"));
  enter(&mut window);
  shows(&mut window, "content");
  let services = window.launch.services().expect("services").clone();
  assert!(services.cloud.is_signed_in());
}
