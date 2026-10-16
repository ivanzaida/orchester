//! Turning two-factor authentication on and off, its codes and recovery codes (`docs/cloud-api.md`, "Two-factor
//! authentication"): the setup takes the password and a code, upgrades only the session that confirmed it, and shows
//! the recovery codes once; a code from the app works once within its window; a recovery code works once; turning it
//! off takes the password and a code. Every change is mailed.

use orchester_api::cloud::{AccountResponse, RecoveryCodes, SteppedUp, TwoFactorStatus};
use orchester_cloud::mail::EmailPurpose;
use serde_json::json;

use crate::support::{PASSWORD, TestCloud, remember};

/// What `GET /me` says of two-factor authentication to the device `token`.
pub(crate) async fn status(cloud: &TestCloud, token: &str) -> TwoFactorStatus {
  let me = cloud.request("GET", "/me", Some(token), None).await;
  assert_eq!(me.status, 200);
  me.json::<AccountResponse>().two_factor
}

/// Waits for an email of `purpose` to `email`; emails sent in the background may arrive in any order.
pub(crate) async fn mailed(cloud: &TestCloud, email: &str, purpose: EmailPurpose) {
  for index in 1.. {
    let sent = cloud.mail.nth(email, index).await;
    if sent.purpose == purpose {
      assert!(sent.body.contains("https://cloud.example.test/account#security"));
      return;
    }
  }
}

pub async fn turning_it_on_takes_the_password_and_a_code_and_upgrades_that_session() {
  let cloud = TestCloud::start().await;
  let email = "setup@example.test";
  cloud.register(email).await;
  let laptop = cloud.sign_in(email, PASSWORD, "Laptop").await;
  let desktop = cloud.sign_in(email, PASSWORD, "Desktop").await;
  let (laptop, desktop) = (laptop.token.expose(), desktop.token.expose());
  a_setup_takes_the_password_and_shows_its_link(&cloud, laptop).await;
  let authenticator = cloud.enable_two_factor(laptop).await;
  assert_eq!(authenticator.recovery_codes.len(), 10);
  for code in &authenticator.recovery_codes {
    assert!(recovery_shaped(code), "{} characters", code.len());
  }
  let on = status(&cloud, laptop).await;
  assert!(on.enabled && on.this_session && on.enabled_at.is_some());
  assert_eq!(on.recovery_codes_left, 10);
  let other = status(&cloud, desktop).await;
  assert!(
    other.enabled && !other.this_session,
    "only the session that turned it on is upgraded"
  );
  mailed(&cloud, email, EmailPurpose::TwoFactorOn).await;
  let again = cloud
    .request(
      "POST",
      "/me/two-factor",
      Some(desktop),
      Some(json!({"password": PASSWORD})),
    )
    .await;
  assert_eq!((again.status, again.error()), (409, "conflict".into()));
  cloud.finish().await;
}

/// A wrong password starts nothing; the right one shows the secret and its `otpauth://` link, and a wrong code does
/// not confirm it.
async fn a_setup_takes_the_password_and_shows_its_link(cloud: &TestCloud, laptop: &str) {
  remember("not the password at all");
  let wrong = cloud
    .request(
      "POST",
      "/me/two-factor",
      Some(laptop),
      Some(json!({"password": "not the password at all"})),
    )
    .await;
  assert_eq!((wrong.status, wrong.error()), (401, "invalid_credentials".into()));
  assert_eq!(status(cloud, laptop).await, TwoFactorStatus::default());
  let setup = cloud.start_two_factor(laptop).await;
  let uri = setup.uri.expose();
  assert!(
    uri.starts_with("otpauth://totp/cloud.example.test:setup%40example.test?secret="),
    "{}",
    uri.len()
  );
  assert!(uri.contains(&format!("secret={}&", setup.secret.expose())));
  assert!(uri.ends_with("&issuer=cloud.example.test&algorithm=SHA1&digits=6&period=30"));
  assert!(!status(cloud, laptop).await.enabled, "not on before a code confirms it");
  let refused = cloud.confirm_two_factor(laptop, "000000").await;
  assert_eq!((refused.status, refused.error()), (401, "invalid_code".into()));
}

/// `xxxxx-xxxxx` in lower-case Crockford base32.
fn recovery_shaped(code: &str) -> bool {
  let alphabet = "0123456789abcdefghjkmnpqrstvwxyz";
  code.len() == 11
    && code.chars().nth(5) == Some('-')
    && code
      .chars()
      .enumerate()
      .all(|(index, c)| index == 5 || alphabet.contains(c))
}

pub async fn a_code_works_once_within_its_window() {
  let cloud = TestCloud::start().await;
  let email = "window@example.test";
  cloud.register(email).await;
  let laptop = cloud.sign_in(email, PASSWORD, "Laptop").await;
  let desktop = cloud.sign_in(email, PASSWORD, "Desktop").await;
  let tablet = cloud.sign_in(email, PASSWORD, "Tablet").await;
  let authenticator = cloud.enable_two_factor(laptop.token.expose()).await;
  let desktop = desktop.token.expose();
  let reused = cloud.step_up(desktop, &authenticator.code(&cloud, 0)).await;
  assert_eq!(
    (reused.status, reused.error()),
    (401, "invalid_code".into()),
    "the code that turned it on is used"
  );
  cloud.clock.advance(90 * 1000);
  let stale = cloud.step_up(desktop, &authenticator.code(&cloud, -2)).await;
  assert_eq!(stale.status, 401, "a minute old is outside the window");
  let early = cloud.step_up(desktop, &authenticator.code(&cloud, 2)).await;
  assert_eq!(early.status, 401, "a minute ahead is outside the window");
  let previous = cloud.step_up(desktop, &authenticator.code(&cloud, -1)).await;
  assert_eq!(previous.status, 200, "the step before is in the window");
  assert_eq!(previous.json::<SteppedUp>().recovery_codes_left, 10);
  assert!(status(&cloud, desktop).await.this_session);
  let tablet = tablet.token.expose();
  let twice = cloud.step_up(tablet, &authenticator.code(&cloud, -1)).await;
  assert_eq!(twice.status, 401, "a code works once");
  let next = cloud.step_up(tablet, &authenticator.code(&cloud, 1)).await;
  assert_eq!(next.status, 200, "the step after is in the window");
  let older = cloud.step_up(tablet, &authenticator.code(&cloud, 0)).await;
  assert_eq!(older.status, 401, "a step before the last one used is refused");
  cloud.finish().await;
}

pub async fn recovery_codes_work_once_and_are_replaced_with_a_code() {
  let cloud = TestCloud::start().await;
  let email = "recovery@example.test";
  cloud.register(email).await;
  let laptop = cloud.sign_in(email, PASSWORD, "Laptop").await;
  let desktop = cloud.sign_in(email, PASSWORD, "Desktop").await;
  let authenticator = cloud.enable_two_factor(laptop.token.expose()).await;
  let (first, second) = (&authenticator.recovery_codes[0], &authenticator.recovery_codes[1]);
  let typed = first.to_uppercase().replace('-', " ");
  let stepped = cloud.step_up(desktop.token.expose(), &typed).await;
  assert_eq!(stepped.status, 200, "case, spaces and dashes do not matter");
  assert_eq!(stepped.json::<SteppedUp>().recovery_codes_left, 9);
  mailed(&cloud, email, EmailPurpose::RecoveryCodeUsed).await;
  let again = cloud.step_up(desktop.token.expose(), first).await;
  assert_eq!((again.status, again.error()), (401, "invalid_code".into()));
  let laptop = laptop.token.expose();
  let made = cloud
    .request(
      "POST",
      "/me/two-factor/recovery-codes",
      Some(laptop),
      Some(json!({"code": "123456"})),
    )
    .await;
  assert_eq!(made.status, 401, "new codes need a right code");
  cloud.clock.advance(30 * 1000);
  let made = cloud
    .request(
      "POST",
      "/me/two-factor/recovery-codes",
      Some(laptop),
      Some(json!({"code": authenticator.code(&cloud, 0)})),
    )
    .await;
  assert_eq!(made.status, 200);
  let fresh: Vec<String> = made
    .json::<RecoveryCodes>()
    .recovery_codes
    .iter()
    .map(|code| {
      remember(code.expose());
      code.expose().to_owned()
    })
    .collect();
  assert_eq!(fresh.len(), 10);
  assert!(!fresh.contains(second));
  assert_eq!(cloud.step_up(laptop, second).await.status, 401, "the old codes ended");
  assert_eq!(status(&cloud, laptop).await.recovery_codes_left, 10);
  assert_eq!(cloud.step_up(laptop, &fresh[0]).await.status, 200);
  cloud.finish().await;
}

pub async fn turning_it_off_takes_the_password_and_a_code() {
  let cloud = TestCloud::start().await;
  let email = "off@example.test";
  cloud.register(email).await;
  let laptop = cloud.sign_in(email, PASSWORD, "Laptop").await;
  let laptop = laptop.token.expose();
  let authenticator = cloud.enable_two_factor(laptop).await;
  remember("not the password at all");
  let off = |password: &str, code: &str| json!({"password": password, "code": code});
  let wrong = cloud
    .request(
      "DELETE",
      "/me/two-factor",
      Some(laptop),
      Some(off("not the password at all", &authenticator.recovery_codes[0])),
    )
    .await;
  assert_eq!((wrong.status, wrong.error()), (401, "invalid_credentials".into()));
  let wrong = cloud
    .request("DELETE", "/me/two-factor", Some(laptop), Some(off(PASSWORD, "000000")))
    .await;
  assert_eq!((wrong.status, wrong.error()), (401, "invalid_code".into()));
  assert!(status(&cloud, laptop).await.enabled);
  let turned = cloud
    .request(
      "DELETE",
      "/me/two-factor",
      Some(laptop),
      Some(off(PASSWORD, &authenticator.recovery_codes[0])),
    )
    .await;
  assert_eq!(turned.status, 204);
  assert_eq!(
    status(&cloud, laptop).await,
    TwoFactorStatus::default(),
    "and the session is no longer one"
  );
  mailed(&cloud, email, EmailPurpose::TwoFactorOff).await;
  let again = cloud
    .request(
      "DELETE",
      "/me/two-factor",
      Some(laptop),
      Some(off(PASSWORD, &authenticator.recovery_codes[1])),
    )
    .await;
  assert_eq!(again.status, 404);
  assert_eq!(
    cloud.try_sign_in(email, PASSWORD, "Laptop").await.status,
    201,
    "a password signs in again"
  );
  let up = cloud.step_up(laptop, &authenticator.recovery_codes[2]).await;
  assert_eq!((up.status, up.error()), (403, "mfa_required".into()));
  cloud.finish().await;
}
