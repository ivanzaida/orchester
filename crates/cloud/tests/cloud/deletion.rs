use orchester_cloud::clock::Clock;
use serde_json::json;

use crate::support::{PASSWORD, TeamServer, TestCloud, postgres, remember};

async fn count(cloud: &TestCloud, sql: &str) -> i64 {
  let admin = postgres().database(&cloud.database).await;
  admin.query_one(sql, &[]).await.expect("count").get(0)
}

/// Deleting asks for the password; then the account, its devices, links, joins and the invites to its email are
/// gone, a server it linked stays with no owner, and the email can sign up again.
pub async fn deleting_the_account_removes_what_the_cloud_holds() {
  let cloud = TestCloud::start().await;
  let email = "leaving@example.test";
  let (browser, desktop, team) = an_account_with_everything(&cloud, email).await;
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
  let again = cloud.sign_in(email, PASSWORD, "Laptop").await;
  assert_eq!(
    cloud.link_server(again.token.expose(), &team).await.status,
    201,
    "the server can be linked again"
  );
  cloud.finish().await;
}

/// An account signed in on two devices, owning a linked server, with an open invite and a reset link; returns the
/// two device tokens and the server.
async fn an_account_with_everything(cloud: &TestCloud, email: &str) -> (String, String, TeamServer) {
  cloud.register(email).await;
  let browser = cloud
    .sign_in(email, PASSWORD, "Browser")
    .await
    .token
    .expose()
    .to_owned();
  let desktop = cloud
    .sign_in(email, PASSWORD, "Desktop")
    .await
    .token
    .expose()
    .to_owned();
  let team = TeamServer::generate();
  assert_eq!(cloud.link_server(&browser, &team).await.status, 201);
  let invite = team.sign_invite(&team.project_invite(email, cloud.clock.now_ms()));
  let routed = cloud
    .request("POST", "/invites", None, Some(json!({"invite": invite})))
    .await;
  assert_eq!(routed.status, 202);
  let reset = cloud
    .request("POST", "/auth/reset", None, Some(json!({"email": email})))
    .await;
  assert_eq!(reset.status, 202);
  cloud.mail.nth(email, 3).await.link_token();
  (browser, desktop, team)
}

/// No row about the account is left; the server it linked stays, with no owner.
async fn nothing_is_left(cloud: &TestCloud) {
  for table in ["accounts", "devices", "email_links", "server_accounts", "invites"] {
    assert_eq!(
      count(cloud, &format!("SELECT count(*) FROM {table}")).await,
      0,
      "{table} is empty"
    );
  }
  assert_eq!(
    count(cloud, "SELECT count(*) FROM servers WHERE owner IS NULL").await,
    1
  );
}

/// Wrong passwords for deleting count against the account like a password change's: five block it for 15 minutes.
pub async fn deleting_with_wrong_passwords_is_limited() {
  let cloud = TestCloud::start().await;
  let email = "guessed@example.test";
  cloud.register(email).await;
  let signed = cloud.sign_in(email, PASSWORD, "Browser").await;
  remember("a guess at the password");
  for _ in 0..5 {
    let wrong = cloud
      .request(
        "DELETE",
        "/me",
        Some(signed.token.expose()),
        Some(json!({"password": "a guess at the password"})),
      )
      .await;
    assert_eq!(wrong.status, 401);
  }
  let blocked = cloud
    .request(
      "DELETE",
      "/me",
      Some(signed.token.expose()),
      Some(json!({"password": PASSWORD})),
    )
    .await;
  assert_eq!((blocked.status, blocked.error()), (429, "rate_limited".into()));
  assert_eq!(
    cloud
      .request("GET", "/me", Some(signed.token.expose()), None)
      .await
      .status,
    200
  );
  cloud.finish().await;
}
