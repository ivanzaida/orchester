//! The gate's code step for an account with two-factor authentication (`Tmwkg` and the states beside it), against the
//! stand-in cloud with codes asked at sign-in: a code from the app or a recovery code finishes the sign-in; a wrong
//! code, too many codes and an ended challenge say so in place; *Back* keeps the email; few recovery codes left are
//! said after the sign-in; and only a person verifies.

use std::time::Duration;

use lurq::app::events::with_synthetic_input;
use orchester_app::routes::{OrchesterView, Route};

use super::{
  has, shows, sign_in, signed_out,
  support::{
    Window,
    signed_in::account_cloud::{EMAIL, PASSWORD, RECOVERY_CODES},
  },
  use_stand_in, value,
};

/// The window at the code step: signed out, the stand-in asking for codes, the password accepted.
fn at_the_code_step() -> Window {
  let mut window = signed_out();
  use_stand_in(&mut window);
  window.data.cloud.require_codes();
  sign_in(&mut window, (EMAIL, PASSWORD));
  shows(&mut window, "gate-code-card");
  window
}

/// Presses `id` with the pointer once it is laid out: a part that just showed may wait for its pass.
fn press(window: &mut Window, id: &str) {
  window.until(&format!("{id} laid out"), |window| {
    window
      .tree
      .get_element_by_id_mut(id)
      .and_then(|element| element.bounds())
      .is_some()
  });
  window.press(id);
}

fn signed_in(window: &Window) -> bool {
  window.launch.services().expect("services").cloud.is_signed_in()
}

#[test]
fn gate_two_factor_signs_in_with_the_apps_code() {
  let mut window = at_the_code_step();
  assert_eq!(window.text("gate-eyebrow"), "TWO-FACTOR AUTHENTICATION");
  assert_eq!(window.text("gate-title"), "Enter your code");
  assert_eq!(window.text("gate-code-email"), EMAIL);
  assert_eq!(window.text("gate-code-accepted"), "Password accepted");
  assert_eq!(window.text("gate-code-label"), "6-digit code");
  assert!(!has(&mut window, "gate-card"), "the code step takes the card's place");
  assert!(!has(&mut window, "orchester-advanced"), "and has no Advanced");
  assert!(!signed_in(&window), "nothing is signed in before the code");

  let code = window.data.cloud.next_code();
  window.set("gate-code", &format!("{} {}", &code[..3], &code[3..]));
  press(&mut window, "gate-verify");
  shows(&mut window, "content");
  assert!(signed_in(&window));
  assert!(!has(&mut window, "screen-sign-in-gate"));
}

#[test]
fn gate_two_factor_says_it_is_verifying() {
  let mut window = at_the_code_step();
  window.data.cloud.slow_sign_in(Duration::from_secs(2));
  let code = window.data.cloud.next_code();
  window.set("gate-code", &code);
  press(&mut window, "gate-verify");
  shows(&mut window, "gate-code-checking");
  let host = format!("localhost:{}", window.data.cloud.address.port());
  assert_eq!(window.text("gate-code-checking"), format!("Checking with {host}…"));
  assert_eq!(window.text("gate-verify"), "Verifying…");
  shows(&mut window, "content");
}

#[test]
fn gate_two_factor_a_wrong_code_is_said_under_the_field() {
  let mut window = at_the_code_step();
  window.set("gate-code", "000000");
  press(&mut window, "gate-verify");
  shows(&mut window, "gate-code-note");
  assert_eq!(
    window.text("gate-code-note"),
    "That code is not right. Enter the newest code from your app."
  );
  assert_eq!(value(&mut window, "gate-code"), "", "the field is cleared");
  assert!(!signed_in(&window));

  window.set("gate-code", "12345");
  press(&mut window, "gate-verify");
  shows(&mut window, "gate-code-note");
  assert_eq!(window.text("gate-code-note"), "A code from your app has 6 digits.");

  let code = window.data.cloud.next_code();
  window.set("gate-code", &code);
  press(&mut window, "gate-verify");
  shows(&mut window, "content");
}

#[test]
fn gate_two_factor_a_recovery_code_signs_in_and_says_few_are_left() {
  let mut window = at_the_code_step();
  window.data.cloud.use_recovery_codes(8);
  press(&mut window, "gate-code-switch");
  shows(&mut window, "gate-code-label");
  window.until("the recovery code's step", |window| {
    window
      .texts("gate-title")
      .is_some_and(|texts| texts.join("") == "Enter a recovery code")
  });
  assert_eq!(window.text("gate-code-label"), "Recovery code");
  assert_eq!(window.text("gate-code-switch"), "Use your authenticator app instead");
  window.set("gate-code", RECOVERY_CODES[8]);
  press(&mut window, "gate-verify");
  shows(&mut window, "content");
  assert!(signed_in(&window));

  window.navigation().go(Route::AllProjects);
  shows(&mut window, "all-projects-recovery");
  assert_eq!(window.text("all-projects-recovery-title"), "1 recovery code left");
  window.navigation().go(Route::Orchester(OrchesterView::default()));
  shows(&mut window, "orchester-cloud-recovery");
  press(&mut window, "orchester-cloud-recovery-security");
  let security = format!("{}/account#security", window.data.cloud.url());
  assert_eq!(
    window.launch.services().expect("services").links.recorded(),
    vec![security]
  );

  // New codes made in the browser: the cloud's count, read again as the panel opens, puts the warning away.
  window.data.cloud.use_recovery_codes(0);
  window.navigation().go(Route::AllProjects);
  shows(&mut window, "all-projects-title");
  window.navigation().go(Route::Orchester(OrchesterView::default()));
  shows(&mut window, "orchester-cloud-details");
  window.until("the warning gone", |window| {
    window.tree.get_element_by_id("orchester-cloud-recovery").is_none()
  });
}

#[test]
fn gate_two_factor_an_ended_challenge_goes_back_to_the_password() {
  let mut window = at_the_code_step();
  window.data.cloud.end_challenges();
  let code = window.data.cloud.next_code();
  window.set("gate-code", &code);
  press(&mut window, "gate-verify");
  shows(&mut window, "gate-timed-out");
  assert_eq!(window.text("gate-timed-out-title"), "Sign-in timed out");
  assert_eq!(
    window.text("gate-timed-out-body"),
    "The code wasn't entered in time. Enter your password again, then your code."
  );
  assert_eq!(value(&mut window, "gate-email"), EMAIL, "the email stays");
  assert_eq!(value(&mut window, "gate-password"), "", "the password is typed again");
  assert_eq!(window.text("gate-title"), "Sign in to start");

  sign_in(&mut window, (EMAIL, PASSWORD));
  shows(&mut window, "gate-code-card");
  assert!(!has(&mut window, "gate-timed-out"));
}

#[test]
fn gate_two_factor_too_many_codes_pause_verify() {
  let mut window = at_the_code_step();
  window.data.cloud.limit_codes(true);
  let code = window.data.cloud.next_code();
  window.set("gate-code", &code);
  press(&mut window, "gate-verify");
  shows(&mut window, "gate-code-paused");
  assert_eq!(window.text("gate-code-paused-title"), "Too many attempts");
  let body = window.text("gate-code-paused-body");
  assert!(
    body.starts_with("Sign-in for this account is paused. Try again at ") && body.ends_with('.'),
    "{body}"
  );
  window.set("gate-code", &code);
  press(&mut window, "gate-verify");
  window.pass();
  assert!(!has(&mut window, "gate-code-checking"), "Verify waits for the pause");
  press(&mut window, "gate-code-back");
  shows(&mut window, "gate-email");
}

#[test]
fn gate_two_factor_back_forgets_a_typed_recovery_code() {
  let mut window = at_the_code_step();
  press(&mut window, "gate-code-switch");
  window.until("the recovery code's step", |window| {
    window
      .texts("gate-code-label")
      .is_some_and(|texts| texts.join("") == "Recovery code")
  });
  window.set("gate-code", RECOVERY_CODES[0]);
  press(&mut window, "gate-code-back");
  sign_in(&mut window, (EMAIL, PASSWORD));
  shows(&mut window, "gate-code-card");
  assert_eq!(window.text("gate-code-label"), "6-digit code");
  assert_eq!(value(&mut window, "gate-code"), "", "the recovery code is gone");
}

#[test]
fn gate_two_factor_a_pause_outlives_its_code_step() {
  let mut window = at_the_code_step();
  window.data.cloud.limit_codes(true);
  let code = window.data.cloud.next_code();
  window.set("gate-code", &code);
  press(&mut window, "gate-verify");
  shows(&mut window, "gate-code-paused");
  press(&mut window, "gate-code-back");
  sign_in(&mut window, (EMAIL, PASSWORD));
  shows(&mut window, "gate-code-card");
  shows(&mut window, "gate-code-paused");
  window.set("gate-code", &code);
  press(&mut window, "gate-verify");
  window.pass();
  assert!(
    !has(&mut window, "gate-code-checking"),
    "Verify still waits for the pause"
  );
}

#[test]
fn gate_two_factor_back_keeps_the_email() {
  let mut window = at_the_code_step();
  press(&mut window, "gate-code-back");
  shows(&mut window, "gate-email");
  assert_eq!(value(&mut window, "gate-email"), EMAIL);
  assert_eq!(value(&mut window, "gate-password"), "");
  assert!(!has(&mut window, "gate-timed-out"), "Back is no timeout");
  assert_eq!(window.text("gate-title"), "Sign in to start");
}

#[test]
fn gate_two_factor_synthetic_input_cannot_verify() {
  let mut window = at_the_code_step();
  let code = window.data.cloud.next_code();
  window.set("gate-code", &code);
  with_synthetic_input(|| press(&mut window, "gate-verify"));
  shows(&mut window, "gate-verify-person-only");
  assert!(
    window
      .text("gate-verify-person-only")
      .contains("Only a person does this here"),
    "the app's person-only reason"
  );
  assert!(!has(&mut window, "gate-code-checking"));
  assert!(!signed_in(&window));
}
