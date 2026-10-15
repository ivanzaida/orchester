use orchester_api::cloud::{AccountResponse, CloudSignedIn, DevicesResponse};
use orchester_cloud::mail::EmailPurpose;
use serde_json::json;

use crate::support::{DAY, HOUR, PASSWORD, TestCloud, postgres, remember};

const NEW_PASSWORD: &str = "a brand new passphrase";

pub async fn reset_signs_out_every_device() {
  let cloud = TestCloud::start().await;
  let email = "reset@example.test";
  cloud.register(email).await;
  let laptop = cloud.sign_in(email, PASSWORD, "Laptop").await;
  let desktop = cloud.sign_in(email, PASSWORD, "Desktop").await;
  assert_eq!(
    cloud
      .request("POST", "/auth/reset", None, Some(json!({"email": email})))
      .await
      .status,
    202
  );
  let sent = cloud.mail.nth(email, 2).await;
  assert_eq!(sent.purpose, EmailPurpose::Reset);
  assert!(sent.body.contains("https://cloud.example.test/reset/"));
  let token = sent.link_token();
  remember(NEW_PASSWORD);
  let short = cloud
    .request(
      "POST",
      "/auth/reset/finish",
      None,
      Some(json!({"link_token": token, "new_password": "short"})),
    )
    .await;
  assert_eq!((short.status, short.error()), (422, "invalid".into()));
  let finish = json!({"link_token": token, "new_password": NEW_PASSWORD});
  assert_eq!(
    cloud
      .request("POST", "/auth/reset/finish", None, Some(finish.clone()))
      .await
      .status,
    204
  );
  for device in [&laptop, &desktop] {
    assert_eq!(
      cloud
        .request("GET", "/me", Some(device.token.expose()), None)
        .await
        .status,
      401
    );
  }
  assert_eq!(cloud.try_sign_in(email, PASSWORD, "Laptop").await.status, 401);
  cloud.sign_in(email, NEW_PASSWORD, "Laptop").await;
  let reused = cloud.request("POST", "/auth/reset/finish", None, Some(finish)).await;
  assert_eq!((reused.status, reused.error()), (422, "invalid_link".into()));
  reset_link_expires(&cloud, email).await;
  cloud.finish().await;
}

async fn reset_link_expires(cloud: &TestCloud, email: &str) {
  assert_eq!(
    cloud
      .request("POST", "/auth/reset", None, Some(json!({"email": email})))
      .await
      .status,
    202
  );
  let late = cloud.mail.nth(email, 3).await.link_token();
  cloud.clock.advance(HOUR + 1000);
  let expired = cloud
    .request(
      "POST",
      "/auth/reset/finish",
      None,
      Some(json!({"link_token": late, "new_password": PASSWORD})),
    )
    .await;
  assert_eq!((expired.status, expired.error()), (422, "invalid_link".into()));
}

pub async fn password_change_signs_out_the_other_devices() {
  let cloud = TestCloud::start().await;
  let email = "change@example.test";
  cloud.register(email).await;
  let laptop = cloud.sign_in(email, PASSWORD, "Laptop").await;
  let desktop = cloud.sign_in(email, PASSWORD, "Desktop").await;
  let token = laptop.token.expose();
  remember(NEW_PASSWORD);
  let wrong = cloud
    .request(
      "POST",
      "/auth/password",
      Some(token),
      Some(json!({"current_password": "not my password at all",
      "new_password": NEW_PASSWORD})),
    )
    .await;
  assert_eq!((wrong.status, wrong.error()), (401, "invalid_credentials".into()));
  let change = json!({"current_password": PASSWORD, "new_password": NEW_PASSWORD});
  assert_eq!(
    cloud
      .request("POST", "/auth/password", Some(token), Some(change))
      .await
      .status,
    204
  );
  assert_eq!(cloud.request("GET", "/me", Some(token), None).await.status, 200);
  assert_eq!(
    cloud
      .request("GET", "/me", Some(desktop.token.expose()), None)
      .await
      .status,
    401
  );
  assert_eq!(cloud.try_sign_in(email, PASSWORD, "Laptop").await.status, 401);
  cloud.sign_in(email, NEW_PASSWORD, "Laptop").await;
  cloud.finish().await;
}

pub async fn devices_are_listed_and_revoked() {
  let cloud = TestCloud::start().await;
  let email = "devices@example.test";
  cloud.register(email).await;
  let laptop = cloud.sign_in(email, PASSWORD, "Laptop").await;
  let desktop = cloud.sign_in(email, PASSWORD, "Desktop").await;
  let browser = cloud.sign_in(email, PASSWORD, "Browser").await;
  let token = laptop.token.expose();
  let listed = cloud
    .request("GET", "/devices", Some(token), None)
    .await
    .json::<DevicesResponse>();
  let names: Vec<_> = listed
    .devices
    .iter()
    .map(|device| (device.name.as_str(), device.current))
    .collect();
  assert_eq!(names, [("Laptop", true), ("Desktop", false), ("Browser", false)]);

  revoke_another_device(&cloud, token, &desktop).await;
  account_changes(&cloud, token).await;

  let browser_token = browser.token.expose();
  assert_eq!(
    cloud
      .request("DELETE", "/auth/sessions/current", Some(browser_token), None)
      .await
      .status,
    204
  );
  assert_eq!(cloud.request("GET", "/me", Some(browser_token), None).await.status, 401);

  cloud.clock.advance(91 * DAY);
  assert_eq!(
    cloud.request("GET", "/me", Some(token), None).await.status,
    401,
    "unused for 90 days"
  );
  cloud.finish().await;
}

/// Revoking another device signs it out; it, or an unknown device, cannot be revoked again.
async fn revoke_another_device(cloud: &TestCloud, token: &str, other: &CloudSignedIn) {
  let path = format!("/devices/{}", other.device.id);
  assert_eq!(cloud.request("DELETE", &path, Some(token), None).await.status, 204);
  assert_eq!(
    cloud
      .request("GET", "/me", Some(other.token.expose()), None)
      .await
      .status,
    401
  );
  assert_eq!(cloud.request("DELETE", &path, Some(token), None).await.status, 404);
  let unknown = format!("/devices/{}", orchester_core::id::CloudDeviceId::generate());
  assert_eq!(cloud.request("DELETE", &unknown, Some(token), None).await.status, 404);
  let listed = cloud
    .request("GET", "/devices", Some(token), None)
    .await
    .json::<DevicesResponse>();
  assert_eq!(listed.devices.len(), 2);
}

/// The display name changes; an empty one and a new email are refused.
async fn account_changes(cloud: &TestCloud, token: &str) {
  let renamed = cloud
    .request("PATCH", "/me", Some(token), Some(json!({"display_name": " Ada King "})))
    .await;
  assert_eq!(renamed.json::<AccountResponse>().account.display_name, "Ada King");
  let empty = cloud
    .request("PATCH", "/me", Some(token), Some(json!({"display_name": " "})))
    .await;
  assert_eq!(empty.status, 422);
  let email_change = cloud
    .request(
      "PATCH",
      "/me",
      Some(token),
      Some(json!({"email": "other@example.test"})),
    )
    .await;
  assert_eq!(email_change.status, 400, "a new email is not accepted here yet");
}

pub async fn web_sessions_end_twelve_hours_after_sign_in() {
  let cloud = TestCloud::start().await;
  let email = "browser@example.test";
  cloud.register(email).await;
  let desktop = cloud.sign_in(email, PASSWORD, "Desktop").await;
  let body = json!({"email": email, "password": PASSWORD, "device": {"name": "Firefox", "platform": "web"}});
  let browser: CloudSignedIn = cloud.request("POST", "/auth/sessions", None, Some(body)).await.json();
  remember(browser.token.expose());
  cloud.clock.advance(11 * HOUR);
  assert_eq!(
    cloud
      .request("GET", "/me", Some(browser.token.expose()), None)
      .await
      .status,
    200
  );
  cloud.clock.advance(HOUR + 1000);
  assert_eq!(
    cloud
      .request("GET", "/me", Some(browser.token.expose()), None)
      .await
      .status,
    401,
    "even though used"
  );
  let listed = cloud
    .request("GET", "/devices", Some(desktop.token.expose()), None)
    .await
    .json::<DevicesResponse>();
  assert_eq!(listed.devices.len(), 1, "the browser's session is gone from the list");
  cloud.finish().await;
}

/// Review 1, M1: a sign-in whose password check passed just before a reset commits must not leave a device behind.
/// The reset here is held open after it changed the password and signed every device out; the sign-in then waits
/// for it and finds the password changed.
pub async fn a_sign_in_racing_a_reset_does_not_survive_it() {
  let cloud = TestCloud::start().await;
  let email = "race@example.test";
  cloud.register(email).await;
  let mut admin = postgres().database(&cloud.database).await;
  let reset = admin.transaction().await.expect("a reset's transaction");
  reset
    .execute(
      "UPDATE accounts SET password_hash = 'set by a reset' WHERE email = $1",
      &[&email],
    )
    .await
    .expect("a new password");
  reset
    .execute(
      "UPDATE devices SET revoked_at = now() FROM accounts a WHERE devices.account = a.id AND a.email = $1",
      &[&email],
    )
    .await
    .expect("every device signed out");
  let commit_later = async move {
    tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
    reset.commit().await.expect("the reset commits");
  };
  let (signed_in, ()) = tokio::join!(cloud.try_sign_in(email, PASSWORD, "Laptop"), commit_later);
  assert_eq!(
    (signed_in.status, signed_in.error()),
    (401, "invalid_credentials".into())
  );
  let left: i64 = admin
    .query_one(
      "SELECT count(*) FROM devices d JOIN accounts a ON a.id = d.account WHERE a.email = $1 AND d.revoked_at IS NULL",
      &[&email],
    )
    .await
    .expect("count devices")
    .get(0);
  assert_eq!(left, 0, "no device outlives the reset");
  drop(admin);
  cloud.finish().await;
}
