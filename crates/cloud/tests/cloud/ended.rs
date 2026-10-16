//! Why a token ended (`docs/cloud-api.md`, "Why a token ended"): the `401` for a device's or a web session's token that
//! the cloud ended names the reason and the time, so the desktop's sign-in gate says what happened.

use orchester_api::cloud::{CloudSignedIn, SessionEnd};
use orchester_cloud::clock::Clock;
use serde_json::{Value, json};

use crate::support::{DAY, PASSWORD, TestCloud, postgres, remember};

const NEW_PASSWORD: &str = "the passphrase after the change";

/// The `401` of `GET /me` with `token`: its reason and its time, `None` without one.
async fn refusal(cloud: &TestCloud, token: &str) -> Option<(SessionEnd, i64)> {
  let answer = cloud.request("GET", "/me", Some(token), None).await;
  assert_eq!((answer.status, answer.error()), (401, "unauthorized".into()));
  let body = answer.json::<Value>();
  let reason = serde_json::from_value::<Option<SessionEnd>>(body["reason"].clone()).expect("a reason or none");
  let at = body["ended_at"].as_i64();
  assert_eq!(reason.is_some(), at.is_some(), "a reason comes with its time: {body}");
  reason.zip(at)
}

/// Signed out from the device itself, from another device, and from the browser: `signed_out`, `revoked`, and a web
/// session ended the same way; a changed and a reset password: `password_reset`.
pub async fn a_refused_token_says_why_it_ended() {
  let cloud = TestCloud::start().await;
  let email = "ended@example.test";
  cloud.register(email).await;
  let [laptop, desktop, tablet] = [
    cloud.sign_in(email, PASSWORD, "Laptop").await,
    cloud.sign_in(email, PASSWORD, "Desktop").await,
    cloud.sign_in(email, PASSWORD, "Tablet").await,
  ];
  let browser = web_session(&cloud, email).await;

  let now = cloud.clock.now_ms();
  let signed_out = cloud
    .request("DELETE", "/auth/sessions/current", Some(desktop.token.expose()), None)
    .await;
  assert_eq!(signed_out.status, 204);
  assert_eq!(
    refusal(&cloud, desktop.token.expose()).await,
    Some((SessionEnd::SignedOut, now))
  );

  revoke(&cloud, &laptop, &browser.device.id.to_string()).await;
  assert_eq!(
    refusal(&cloud, browser.token.expose()).await,
    Some((SessionEnd::Revoked, now))
  );

  remember(NEW_PASSWORD);
  let change = json!({"current_password": PASSWORD, "new_password": NEW_PASSWORD});
  let changed = cloud
    .request("POST", "/auth/password", Some(laptop.token.expose()), Some(change))
    .await;
  assert_eq!(changed.status, 204);
  assert_eq!(
    refusal(&cloud, tablet.token.expose()).await,
    Some((SessionEnd::PasswordReset, now))
  );
  assert_eq!(
    cloud
      .request("GET", "/me", Some(laptop.token.expose()), None)
      .await
      .status,
    200
  );

  cloud.clock.advance(1000);
  reset(&cloud, email).await;
  assert_eq!(
    refusal(&cloud, laptop.token.expose()).await,
    Some((SessionEnd::PasswordReset, now + 1000))
  );
  cloud.finish().await;
}

/// A deleted account's tokens say so until they would have ended unused, then no more; a token that ended unused, or
/// one the cloud never issued, says nothing.
pub async fn tokens_of_a_deleted_account_say_so_until_they_would_have_ended() {
  let cloud = TestCloud::start().await;
  let email = "deleted@example.test";
  cloud.register(email).await;
  let laptop = cloud.sign_in(email, PASSWORD, "Laptop").await;
  let desktop = cloud.sign_in(email, PASSWORD, "Desktop").await;
  let now = cloud.clock.now_ms();
  let deleted = cloud
    .request(
      "DELETE",
      "/me",
      Some(laptop.token.expose()),
      Some(json!({"password": PASSWORD})),
    )
    .await;
  assert_eq!(deleted.status, 204);
  for device in [&laptop, &desktop] {
    assert_eq!(
      refusal(&cloud, device.token.expose()).await,
      Some((SessionEnd::AccountDeleted, now))
    );
  }
  let admin = postgres().database(&cloud.database).await;
  let kept = kept_hashes(&admin).await;
  assert_eq!(kept, 2, "only the two tokens' hashes are kept");

  let unknown = format!("orcc_{}", "0".repeat(64));
  assert_eq!(refusal(&cloud, &unknown).await, None);

  cloud.clock.advance(90 * DAY + 1);
  cloud.cloud.sweep().await.expect("sweep");
  assert_eq!(refusal(&cloud, desktop.token.expose()).await, None);
  let kept = kept_hashes(&admin).await;
  assert_eq!(kept, 0, "the sweep removed them");

  cloud.register("idle@example.test").await;
  let idle = cloud.sign_in("idle@example.test", PASSWORD, "Idle").await;
  cloud.clock.advance(91 * DAY);
  assert_eq!(refusal(&cloud, idle.token.expose()).await, None, "ended unused");
  cloud.finish().await;
}

/// A token that had ended before its account was deleted, here a browser's after its 12 hours, says nothing, and its
/// hash is not kept.
pub async fn tokens_ended_before_the_account_was_deleted_say_nothing() {
  let cloud = TestCloud::start().await;
  let email = "ended-first@example.test";
  cloud.register(email).await;
  let browser = web_session(&cloud, email).await;
  cloud.clock.advance(DAY / 2 + 1);
  let laptop = cloud.sign_in(email, PASSWORD, "Laptop").await;
  let deleted = cloud
    .request(
      "DELETE",
      "/me",
      Some(laptop.token.expose()),
      Some(json!({"password": PASSWORD})),
    )
    .await;
  assert_eq!(deleted.status, 204);
  assert_eq!(
    refusal(&cloud, laptop.token.expose()).await.map(|(reason, _)| reason),
    Some(SessionEnd::AccountDeleted)
  );
  assert_eq!(refusal(&cloud, browser.token.expose()).await, None, "it had ended");
  let admin = postgres().database(&cloud.database).await;
  assert_eq!(kept_hashes(&admin).await, 1, "only the token still in use is kept");
  cloud.finish().await;
}

/// How many token hashes the cloud keeps for deleted accounts.
async fn kept_hashes(admin: &tokio_postgres::Client) -> i64 {
  admin
    .query_one("SELECT count(*) FROM ended_tokens", &[])
    .await
    .expect("count")
    .get(0)
}

async fn web_session(cloud: &TestCloud, email: &str) -> CloudSignedIn {
  let body = json!({"email": email, "password": PASSWORD, "device": {"name": "Firefox", "platform": "web"}});
  let answer = cloud.request("POST", "/auth/sessions", None, Some(body)).await;
  assert_eq!(answer.status, 201);
  let signed: CloudSignedIn = answer.json();
  remember(signed.token.expose());
  signed
}

async fn revoke(cloud: &TestCloud, by: &CloudSignedIn, device: &str) {
  let path = format!("/devices/{device}");
  let answer = cloud.request("DELETE", &path, Some(by.token.expose()), None).await;
  assert_eq!(answer.status, 204);
}

/// Resets the password of `email` through its emailed link.
async fn reset(cloud: &TestCloud, email: &str) {
  let asked = cloud
    .request("POST", "/auth/reset", None, Some(json!({"email": email})))
    .await;
  assert_eq!(asked.status, 202);
  let sent = cloud.mail.nth(email, 2).await;
  remember(PASSWORD);
  let finish = json!({"link_token": sent.link_token(), "new_password": PASSWORD});
  let finished = cloud.request("POST", "/auth/reset/finish", None, Some(finish)).await;
  assert_eq!(finished.status, 204);
}
