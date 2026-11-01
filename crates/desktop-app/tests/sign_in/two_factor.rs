//! The sign-in's code step for an account with two-factor authentication (`eWsUb`), against the stand-in cloud with
//! codes asked at sign-in: a code from the app or a recovery code finishes the sign-in; a wrong code, too many codes
//! and an ended challenge say so in place; *Back* keeps the email; few recovery codes left are said after the sign-in;
//! and only a person verifies.

use std::time::Duration;

use lurq::app::events::with_synthetic_input;
use orchester_app::routes::{OrchesterView, Route};

use super::{
  ACCOUNT_CARD, enter, focus_on, has, shows, sign_in, signed_out,
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
  shows(&mut window, "sign-in-code-card");
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
fn sign_in_two_factor_signs_in_with_the_apps_code_in_the_same_card() {
  let mut window = at_the_code_step();
  assert_eq!(window.text("sign-in-title"), "Enter your code");
  assert_eq!(window.text("sign-in-code-label"), "6-digit code");
  assert!(
    !has(&mut window, "sign-in-form"),
    "the code step takes the fields' place"
  );
  assert!(has(&mut window, "orchester-advanced"), "the rest of the page stays");
  assert!(!signed_in(&window), "nothing is signed in before the code");

  let code = window.data.cloud.next_code();
  window.set("sign-in-code", &format!("{} {}", &code[..3], &code[3..]));
  assert_eq!(
    window.text("sign-in-code-box-0"),
    code[..1],
    "the boxes show what is typed"
  );
  press(&mut window, "sign-in-verify");
  shows(&mut window, ACCOUNT_CARD);
  assert!(signed_in(&window));
}

#[test]
fn sign_in_two_factor_tab_and_enter_go_through_the_code_step() {
  let mut window = at_the_code_step();
  focus_on(&mut window, "sign-in-code");
  window.set("sign-in-code", "000000");
  enter(&mut window);
  shows(&mut window, "sign-in-code-note");
  let code = window.data.cloud.next_code();
  window.set("sign-in-code", &code);
  with_synthetic_input(|| enter(&mut window));
  shows(&mut window, "sign-in-verify-person-only");
  assert!(!has(&mut window, "sign-in-code-checking"));
  enter(&mut window);
  shows(&mut window, ACCOUNT_CARD);
}

#[test]
fn sign_in_two_factor_says_it_is_verifying() {
  let mut window = at_the_code_step();
  window.data.cloud.slow_sign_in(Duration::from_secs(2));
  let code = window.data.cloud.next_code();
  window.set("sign-in-code", &code);
  press(&mut window, "sign-in-verify");
  shows(&mut window, "sign-in-code-checking");
  assert_eq!(window.text("sign-in-verify"), "Verifying…");
  shows(&mut window, ACCOUNT_CARD);
}

#[test]
fn sign_in_two_factor_a_wrong_code_is_said_under_the_boxes() {
  let mut window = at_the_code_step();
  window.set("sign-in-code", "000000");
  press(&mut window, "sign-in-verify");
  shows(&mut window, "sign-in-code-note");
  assert_eq!(
    window.text("sign-in-code-note"),
    "That code is not right. Enter the newest code from your app."
  );
  assert_eq!(value(&mut window, "sign-in-code"), "", "the code is cleared");
  assert!(!signed_in(&window));

  window.set("sign-in-code", "12345");
  press(&mut window, "sign-in-verify");
  shows(&mut window, "sign-in-code-note");
  assert_eq!(window.text("sign-in-code-note"), "A code from your app has 6 digits.");

  let code = window.data.cloud.next_code();
  window.set("sign-in-code", &code);
  press(&mut window, "sign-in-verify");
  shows(&mut window, ACCOUNT_CARD);
}

#[test]
fn sign_in_two_factor_a_recovery_code_signs_in_and_says_few_are_left() {
  let mut window = at_the_code_step();
  window.data.cloud.use_recovery_codes(8);
  press(&mut window, "sign-in-code-switch");
  window.until("the recovery code's step", |window| {
    window
      .texts("sign-in-title")
      .is_some_and(|texts| texts.join("") == "Enter a recovery code")
  });
  assert_eq!(window.text("sign-in-code-label"), "Recovery code");
  assert_eq!(window.text("sign-in-code-switch"), "Use your authenticator app instead");
  window.set("sign-in-code", RECOVERY_CODES[8]);
  press(&mut window, "sign-in-verify");
  shows(&mut window, ACCOUNT_CARD);
  assert!(signed_in(&window));

  window.navigation().go(Route::AllProjects);
  shows(&mut window, "all-projects-recovery");
  assert_eq!(window.text("all-projects-recovery-title"), "1 recovery code left");
  window.navigation().go(Route::Orchester(OrchesterView::default()));
  shows(&mut window, "orchester-cloud-recovery");
}

#[test]
fn sign_in_two_factor_an_ended_challenge_goes_back_to_the_password() {
  let mut window = at_the_code_step();
  window.data.cloud.end_challenges();
  let code = window.data.cloud.next_code();
  window.set("sign-in-code", &code);
  press(&mut window, "sign-in-verify");
  shows(&mut window, "sign-in-timed-out");
  assert_eq!(window.text("sign-in-timed-out-title"), "Sign-in timed out");
  assert_eq!(value(&mut window, "sign-in-email"), EMAIL, "the email stays");
  assert_eq!(
    value(&mut window, "sign-in-password"),
    "",
    "the password is typed again"
  );

  sign_in(&mut window, (EMAIL, PASSWORD));
  shows(&mut window, "sign-in-code-card");
  assert!(!has(&mut window, "sign-in-timed-out"));
}

#[test]
fn sign_in_two_factor_too_many_codes_pause_verify_past_its_step() {
  let mut window = at_the_code_step();
  window.data.cloud.limit_codes(true);
  let code = window.data.cloud.next_code();
  window.set("sign-in-code", &code);
  press(&mut window, "sign-in-verify");
  shows(&mut window, "sign-in-code-paused");
  assert_eq!(window.text("sign-in-code-paused-title"), "Too many attempts");
  press(&mut window, "sign-in-code-back");
  sign_in(&mut window, (EMAIL, PASSWORD));
  shows(&mut window, "sign-in-code-card");
  shows(&mut window, "sign-in-code-paused");
  window.set("sign-in-code", &code);
  press(&mut window, "sign-in-verify");
  window.pass();
  assert!(
    !has(&mut window, "sign-in-code-checking"),
    "Verify still waits for the pause"
  );
}

#[test]
fn sign_in_two_factor_back_keeps_the_email_and_forgets_a_typed_code() {
  let mut window = at_the_code_step();
  press(&mut window, "sign-in-code-switch");
  window.until("the recovery code's step", |window| {
    window
      .texts("sign-in-code-label")
      .is_some_and(|texts| texts.join("") == "Recovery code")
  });
  window.set("sign-in-code", RECOVERY_CODES[0]);
  press(&mut window, "sign-in-code-back");
  shows(&mut window, "sign-in-email");
  assert_eq!(value(&mut window, "sign-in-email"), EMAIL);
  assert_eq!(value(&mut window, "sign-in-password"), "");
  assert!(!has(&mut window, "sign-in-timed-out"), "Back is no timeout");
  sign_in(&mut window, (EMAIL, PASSWORD));
  shows(&mut window, "sign-in-code-card");
  assert_eq!(window.text("sign-in-code-label"), "6-digit code");
  assert_eq!(value(&mut window, "sign-in-code"), "", "the recovery code is gone");
}

#[test]
fn sign_in_two_factor_synthetic_input_cannot_verify() {
  let mut window = at_the_code_step();
  let code = window.data.cloud.next_code();
  window.set("sign-in-code", &code);
  with_synthetic_input(|| press(&mut window, "sign-in-verify"));
  shows(&mut window, "sign-in-verify-person-only");
  assert!(
    window
      .text("sign-in-verify-person-only")
      .contains("Only a person does this here"),
    "the app's person-only reason"
  );
  assert!(!has(&mut window, "sign-in-code-checking"));
  assert!(!signed_in(&window));
}
