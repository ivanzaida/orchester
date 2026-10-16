//! What guards two-factor authentication (`docs/cloud-api.md`, "Two-factor authentication", "Password change"): a
//! daily cap on wrong codes that the quarter-hour limit does not lift, mailed at the tenth; a code spent once however
//! many requests race with it; a challenge that spends one code when two finishes race; and the changes that a
//! password alone does not make while two-factor authentication is on.

use orchester_api::cloud::SteppedUp;
use orchester_cloud::mail::EmailPurpose;
use serde_json::json;

use crate::{
  support::{MINUTE, PASSWORD, TestCloud, http::Response, remember, two_factor::Authenticator},
  two_factor::{mailed, status},
};

pub async fn wrong_codes_are_capped_a_day_and_the_tenth_is_mailed() {
  let cloud = TestCloud::start().await;
  let email = "guesses@example.test";
  cloud.register(email).await;
  let laptop = cloud.sign_in(email, PASSWORD, "Laptop").await;
  let token = laptop.token.expose();
  let authenticator = cloud.enable_two_factor(token).await;
  for round in 0..4 {
    for _ in 0..5 {
      let wrong = cloud.step_up(token, "000000").await;
      assert_eq!(wrong.error(), "invalid_code", "round {round}");
    }
    cloud.clock.advance(15 * MINUTE + 1000);
  }
  mailed(&cloud, email, EmailPurpose::TwoFactorFailures).await;
  let capped = cloud.step_up(token, &authenticator.code(&cloud, 0)).await;
  assert_eq!(
    (capped.status, capped.error()),
    (429, "rate_limited".into()),
    "twenty wrong codes a day, whatever the quarter hour"
  );
  let retry: i64 = capped.headers["retry-after"].parse().expect("seconds");
  assert!(retry > 15 * 60, "until the day ends: {retry} s");
  cloud.clock.advance(retry * 1000);
  let after = cloud.step_up(token, &authenticator.code(&cloud, 0)).await;
  assert_eq!(after.status, 200, "a new day");
  cloud.finish().await;
}

/// Two step-ups with `code` at once: one takes it, the other is refused.
async fn raced(cloud: &TestCloud, token: &str, code: &str) {
  let (first, second) = tokio::join!(cloud.step_up(token, code), cloud.step_up(token, code));
  let said = |answer: &Response| match answer.status {
    200 => (200, String::new()),
    status => (status, answer.error()),
  };
  let mut answers = [said(&first), said(&second)];
  answers.sort();
  assert_eq!(answers, [(200, String::new()), (401, "invalid_code".into())]);
}

pub async fn a_code_is_spent_once_by_requests_that_race() {
  let cloud = TestCloud::start().await;
  let email = "race@example.test";
  cloud.register(email).await;
  let laptop = cloud.sign_in(email, PASSWORD, "Laptop").await;
  let token = laptop.token.expose();
  let authenticator = cloud.enable_two_factor(token).await;
  cloud.clock.advance(30 * 1000);
  raced(&cloud, token, &authenticator.code(&cloud, 0)).await;
  raced(&cloud, token, &authenticator.recovery_codes[0]).await;
  assert_eq!(status(&cloud, token).await.recovery_codes_left, 9);
  challenge_finishes_race(&cloud, (email, token), &authenticator).await;
  cloud.finish().await;
}

/// Two finishes of one challenge at once, one with a code of the app and one with a recovery code: one signs in, the
/// other finds the challenge used, and its code stays unspent.
async fn challenge_finishes_race(cloud: &TestCloud, (email, token): (&str, &str), authenticator: &Authenticator) {
  let challenge = cloud.challenge(email, "Desktop").await;
  cloud.clock.advance(30 * 1000);
  let (app, recovery) = (authenticator.code(cloud, 0), &authenticator.recovery_codes[1]);
  let (by_app, by_recovery) = tokio::join!(
    cloud.finish_sign_in(&challenge, &app),
    cloud.finish_sign_in(&challenge, recovery)
  );
  let mut statuses = [by_app.status, by_recovery.status];
  statuses.sort_unstable();
  assert_eq!(statuses, [201, 401]);
  let lost = if by_app.status == 201 { &by_recovery } else { &by_app };
  assert_eq!(lost.error(), "challenge_expired");
  let left = status(cloud, token).await.recovery_codes_left;
  if by_app.status == 201 {
    assert_eq!(left, 9, "the recovery code was not spent");
  } else {
    assert_eq!(left, 8);
    let stepped = cloud.step_up(token, &app).await;
    assert_eq!(stepped.status, 200, "the app's code was not spent");
    assert_eq!(stepped.json::<SteppedUp>().recovery_codes_left, 8);
  }
}

pub async fn a_password_alone_changes_nothing_sensitive_while_it_is_on() {
  let cloud = TestCloud::start().await;
  let email = "sensitive@example.test";
  cloud.register(email).await;
  let laptop = cloud.sign_in(email, PASSWORD, "Laptop").await;
  let desktop = cloud.sign_in(email, PASSWORD, "Desktop").await;
  let (laptop_token, desktop_token) = (laptop.token.expose(), desktop.token.expose());
  let authenticator = cloud.enable_two_factor(laptop_token).await;
  let new_password = "another long passphrase";
  remember(new_password);
  let laptop_path = format!("/devices/{}", laptop.device.id);
  let changes = [
    (
      "POST",
      "/auth/password",
      Some(json!({"current_password": PASSWORD, "new_password": new_password})),
    ),
    ("DELETE", "/me", Some(json!({"password": PASSWORD}))),
    ("DELETE", laptop_path.as_str(), None),
  ];
  for (method, path, body) in changes {
    let refused = cloud.request(method, path, Some(desktop_token), body).await;
    assert_eq!(
      (refused.status, refused.error()),
      (403, "session_without_mfa".into()),
      "{method} {path}"
    );
  }
  cloud.clock.advance(30 * 1000);
  let stepped = cloud.step_up(desktop_token, &authenticator.code(&cloud, 0)).await;
  assert_eq!(stepped.status, 200);
  let revoked = cloud.request("DELETE", &laptop_path, Some(desktop_token), None).await;
  assert_eq!(revoked.status, 204, "a two-factor session signs another device out");
  cloud.finish().await;
}
