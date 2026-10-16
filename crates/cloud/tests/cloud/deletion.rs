use orchester_cloud::clock::Clock;
use orchester_core::id::InviteId;
use serde_json::json;

use crate::{
  registry::{TeamServer, link, signed_in},
  support::{PASSWORD, TestCloud, postgres, remember},
};

async fn count(cloud: &TestCloud, sql: &str) -> i64 {
  let admin = postgres().database(&cloud.database).await;
  admin.query_one(sql, &[]).await.expect("count").get(0)
}

/// Deleting asks for the password; then the account, its devices, links and joins, the servers it linked and the
/// invites to its email are gone, and the email can sign up again.
pub async fn deleting_the_account_removes_what_the_cloud_holds() {
  let cloud = TestCloud::start().await;
  cloud.publish_keys().await;
  let email = "leaving@example.test";
  let (browser, desktop) = an_account_with_everything(&cloud, email).await;
  remember("not my password at all");
  let wrong = cloud
    .request(
      "DELETE",
      "/me",
      Some(&browser),
      Some(json!({"password": "not my password at all"})),
    )
    .await;
  assert_eq!((wrong.status, wrong.error()), (401, "invalid_credentials".into()));
  assert_eq!(
    cloud.request("GET", "/me", Some(&browser), None).await.status,
    200,
    "nothing was deleted"
  );
  assert_eq!(
    cloud
      .request("DELETE", "/me", Some(&browser), Some(json!({})))
      .await
      .status,
    400
  );
  let deleted = cloud
    .request("DELETE", "/me", Some(&browser), Some(json!({"password": PASSWORD})))
    .await;
  assert_eq!(deleted.status, 204);
  for token in [&browser, &desktop] {
    assert_eq!(cloud.request("GET", "/me", Some(token), None).await.status, 401);
  }
  let gone = cloud.try_sign_in(email, PASSWORD, "Laptop").await;
  assert_eq!((gone.status, gone.error()), (401, "invalid_credentials".into()));
  nothing_is_left(&cloud).await;
  cloud.register(email).await;
  assert_eq!(cloud.sign_in(email, PASSWORD, "Laptop").await.account.email, email);
  cloud.finish().await;
}

/// An account signed in on two devices, owning a linked server, with an open invite and a reset link; returns the
/// two device tokens.
async fn an_account_with_everything(cloud: &TestCloud, email: &str) -> (String, String) {
  let browser = signed_in(cloud, email).await;
  let desktop = cloud
    .sign_in(email, PASSWORD, "Desktop")
    .await
    .token
    .expose()
    .to_owned();
  let server = TeamServer::generate();
  link(cloud, &browser, &server).await;
  let invite = server.invite(InviteId::generate(), email, cloud.clock.now_ms());
  let routed = cloud
    .request("POST", "/invites", None, Some(json!({"invite": invite})))
    .await;
  assert_eq!(routed.status, 202);
  let reset = cloud
    .request("POST", "/auth/reset", None, Some(json!({"email": email})))
    .await;
  assert_eq!(reset.status, 202);
  cloud.mail.nth(email, 3).await.link_token();
  (browser, desktop)
}

/// No row about the account is left, nor the server it linked (`servers.owner` cascades).
async fn nothing_is_left(cloud: &TestCloud) {
  for table in [
    "accounts",
    "devices",
    "email_links",
    "server_accounts",
    "servers",
    "invites",
  ] {
    assert_eq!(
      count(cloud, &format!("SELECT count(*) FROM {table}")).await,
      0,
      "{table} is empty"
    );
  }
}

/// Wrong passwords for deleting count against the account like a password change's: five block it for 15 minutes.
pub async fn deleting_with_wrong_passwords_is_limited() {
  let cloud = TestCloud::start().await;
  let email = "guessed@example.test";
  let token = signed_in(&cloud, email).await;
  remember("a guess at the password");
  for _ in 0..5 {
    let wrong = cloud
      .request(
        "DELETE",
        "/me",
        Some(&token),
        Some(json!({"password": "a guess at the password"})),
      )
      .await;
    assert_eq!(wrong.status, 401);
  }
  let blocked = cloud
    .request("DELETE", "/me", Some(&token), Some(json!({"password": PASSWORD})))
    .await;
  assert_eq!((blocked.status, blocked.error()), (429, "rate_limited".into()));
  assert_eq!(cloud.request("GET", "/me", Some(&token), None).await.status, 200);
  cloud.finish().await;
}
