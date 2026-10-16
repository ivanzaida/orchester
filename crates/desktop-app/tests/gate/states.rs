//! The gate's refusals in place (`Atym1`, `vovth`, `t2C4a`, `i8HD8`) and its signing-in state (`U0v7kx`), against the
//! stand-in cloud; the cloud's pages open in the browser (recorded).

use std::time::Duration;

use super::{has, shows, sign_in, signed_out, support::signed_in::account_cloud, use_stand_in, value};

#[test]
fn gate_says_a_wrong_email_or_password_in_place() {
  let mut window = signed_out();
  use_stand_in(&mut window);
  sign_in(&mut window, (account_cloud::EMAIL, "not the password"));
  shows(&mut window, "gate-wrong");
  assert_eq!(
    window.text("gate-wrong"),
    "The email or password is not right. Check both, or reset your password."
  );
  assert_eq!(
    value(&mut window, "gate-email"),
    account_cloud::EMAIL,
    "the email stays"
  );
  assert_eq!(value(&mut window, "gate-password"), "", "the password is typed again");
  assert!(has(&mut window, "screen-sign-in-gate"));
}

#[test]
fn gate_asks_to_confirm_the_email_first() {
  let mut window = signed_out();
  use_stand_in(&mut window);
  sign_in(&mut window, (account_cloud::UNCONFIRMED, "any password at all"));
  shows(&mut window, "gate-unconfirmed");
  assert_eq!(window.text("gate-unconfirmed-title"), "Confirm your email first");
  assert_eq!(
    window.text("gate-unconfirmed-body"),
    format!(
      "Open the link Orchester Cloud sent to {}, then sign in here. No email? Ask for a new one on the cloud's page.",
      account_cloud::UNCONFIRMED
    )
  );
  window.press("gate-open-cloud");
  window.press("gate-create-account");
  window.press("gate-forgot");
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
fn gate_pauses_an_email_with_too_many_attempts() {
  let mut window = signed_out();
  use_stand_in(&mut window);
  sign_in(&mut window, (account_cloud::PAUSED, "any password at all"));
  shows(&mut window, "gate-paused");
  assert_eq!(window.text("gate-paused-title"), "Too many attempts");
  let body = window.text("gate-paused-body");
  assert!(
    body.starts_with("Sign-in for this email is paused. Try again at ") && body.ends_with('.'),
    "{body}"
  );
  window.press("gate-sign-in");
  assert!(
    !has(&mut window, "gate-checking"),
    "Sign in waits until the pause is over"
  );
  window.set("gate-email", account_cloud::EMAIL);
  window.set("gate-password", account_cloud::PASSWORD);
  window.press("gate-sign-in");
  shows(&mut window, "content");
}

#[test]
fn gate_says_the_internet_is_needed_once() {
  let mut window = signed_out();
  use_stand_in(&mut window);
  window.data.cloud.set_down(true);
  sign_in(&mut window, (account_cloud::EMAIL, account_cloud::PASSWORD));
  shows(&mut window, "gate-unreachable");
  assert_eq!(
    window.text("gate-unreachable-title"),
    "Signing in needs the internet once"
  );
  assert_eq!(
    window.text("gate-unreachable-body"),
    "Orchester Cloud can't be reached from this computer. Connect to the internet, then sign in. After this first \
     sign-in, Orchester works offline."
  );
  let host = format!("localhost:{}", window.data.cloud.address.port());
  let note = window.text("gate-unreachable-note");
  assert!(note.starts_with(&format!("{host} didn't answer · ")), "{note}");
  assert!(
    window.text("gate-footer-cloud").ends_with("· test cloud"),
    "a test cloud says so"
  );
}

#[test]
fn gate_says_it_is_signing_in() {
  let mut window = signed_out();
  use_stand_in(&mut window);
  window.data.cloud.slow_sign_in(Duration::from_secs(2));
  sign_in(&mut window, (account_cloud::EMAIL, account_cloud::PASSWORD));
  shows(&mut window, "gate-checking");
  let host = format!("localhost:{}", window.data.cloud.address.port());
  assert_eq!(window.text("gate-checking"), format!("Checking with {host}…"));
  assert_eq!(window.text("gate-sign-in"), "Signing in…");
  shows(&mut window, "content");
}
