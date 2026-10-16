use orchester_api::cloud::{CloudInvites, ServersResponse};
use orchester_cloud::clock::Clock;
use orchester_core::id::InviteId;
use serde_json::json;

use crate::{
  registry::{TeamServer, link, signed_in},
  support::{MINUTE, PASSWORD, TestCloud, postgres, remember},
};

async fn count(cloud: &TestCloud, sql: &str) -> i64 {
  let admin = postgres().database(&cloud.database).await;
  admin.query_one(sql, &[]).await.expect("count").get(0)
}

/// Deleting asks for the password; then the account, its devices, links and joins and the invites to its email are
/// gone, the server it linked stays registered with no owner, and the email can sign up again.
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

/// No row about the account is left; the server it linked stays, with no owner (migration 5).
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

/// Security review of this branch, minor 1: deleting the account that linked a server keeps the server registered,
/// with its members' joins and its invites; the next statement the server signs links it to whoever carries it.
pub async fn a_deleted_owners_server_stays_for_its_members() {
  let cloud = TestCloud::start().await;
  cloud.publish_keys().await;
  let owner = signed_in(&cloud, "owner@example.test").await;
  let member = signed_in(&cloud, "member@example.test").await;
  let invitee = signed_in(&cloud, "invitee@example.test").await;
  let server = TeamServer::generate();
  let id = link(&cloud, &owner, &server).await.server.server.id;
  let joined = cloud
    .request("POST", "/identity-tokens", Some(&member), Some(json!({"audience": id})))
    .await;
  assert_eq!(joined.status, 201);
  remember(
    joined.json::<serde_json::Value>()["identity_token"]
      .as_str()
      .expect("a token"),
  );
  let invite = server.invite(InviteId::generate(), "invitee@example.test", cloud.clock.now_ms());
  let delivered = cloud
    .request("POST", "/invites", None, Some(json!({"invite": invite})))
    .await;
  assert_eq!(delivered.status, 202);
  let deleted = cloud
    .request("DELETE", "/me", Some(&owner), Some(json!({"password": PASSWORD})))
    .await;
  assert_eq!(deleted.status, 204);
  still_there_without_an_owner(&cloud, &server, &member, &invitee).await;
  cloud.clock.advance(MINUTE);
  let admin = signed_in(&cloud, "new.admin@example.test").await;
  link(&cloud, &admin, &server).await;
  let owned: ServersResponse = cloud.request("GET", "/servers", Some(&admin), None).await.json();
  assert!(
    owned.servers.len() == 1 && owned.servers[0].owner,
    "a new statement takes ownership"
  );
  cloud.finish().await;
}

/// After its owner left: the member still lists the server, the invitee still has the invite, and the server still
/// delivers new ones.
async fn still_there_without_an_owner(cloud: &TestCloud, server: &TeamServer, member: &str, invitee: &str) {
  let theirs: ServersResponse = cloud.request("GET", "/servers", Some(member), None).await.json();
  assert_eq!(theirs.servers.len(), 1, "the member still has the server");
  assert!(!theirs.servers[0].owner);
  let open: CloudInvites = cloud.request("GET", "/invites", Some(invitee), None).await.json();
  assert_eq!(open.invites.len(), 1, "the invite is still open");
  let again = server.invite(InviteId::generate(), "invitee@example.test", cloud.clock.now_ms());
  let ownerless = cloud
    .request("POST", "/invites", None, Some(json!({"invite": again})))
    .await;
  assert_eq!(ownerless.status, 202, "an ownerless server still delivers invites");
}
