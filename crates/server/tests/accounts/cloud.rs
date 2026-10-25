use orchester_api::{
  ApiError, CloudLinkResponse, Credential, ErrorCode, InviteCreated, Joined, JoinedVia, MemberRole, ProjectCreated,
  ServerInfo, UserResponse,
};
use orchester_server::host::CloudCommand;
use serde_json::{Value, json};

use crate::support::{
  Response, TestServer,
  cloud::{DAY, Identity, Key, TestCloud},
};

/// A server whose first admin linked it to a test cloud; returns the admin's token and the server's id.
pub(crate) async fn linked() -> (TestServer, Joined, TestCloud, String) {
  let server = TestServer::start(|_| {}).await;
  let admin = server.bootstrap().await;
  let cloud = server.cloud();
  let started: CloudLinkResponse = server
    .request(
      "PATCH",
      "/server/cloud",
      Some(&admin.token),
      Some(json!({"link":"start"})),
    )
    .await
    .json();
  let statement = started.statement.expect("signed link statement");
  assert_eq!(statement.split('.').count(), 3);
  assert!(started.cloud.root_fingerprint.is_none());
  let finished: CloudLinkResponse = server
    .request(
      "PATCH",
      "/server/cloud",
      Some(&admin.token),
      Some(json!({"link": {"root_key": cloud.root.public(), "key_set": cloud.key_set(server.seconds())}})),
    )
    .await
    .json();
  assert_eq!(
    finished.cloud.root_fingerprint.as_deref(),
    Some(cloud.root.fingerprint().as_str())
  );
  assert!(finished.cloud.accepting);
  let audience = finished.cloud.server;
  (server, admin, cloud, audience)
}

pub(crate) async fn cloud_join(server: &TestServer, token: &str, key_set: Option<&str>) -> Response {
  let mut body = json!({"identity_token": token, "device": "Bob's laptop"});
  if let Some(key_set) = key_set {
    body["key_set"] = json!(key_set);
  }
  server.request("POST", "/auth/cloud", None, Some(body)).await
}

pub(crate) fn refused(response: &Response, status: u16, code: ErrorCode) {
  assert_eq!(response.status, status);
  assert_eq!(response.json::<ApiError>().error, code);
}

pub(crate) fn bob<'a>(audience: &'a str, invite: Option<&'a str>) -> Identity<'a> {
  Identity {
    audience,
    subject: "0191c7c0-0000-7000-8000-00000000b0b0",
    email: "Bob@Example.com",
    name: "Bob Koval",
    invite,
  }
}

async fn project(server: &TestServer, token: &Credential, slug: &str) -> ProjectCreated {
  let body = json!({"idempotency_key": orchester_core::id::IdempotencyKey::generate(), "name": slug, "slug": slug,
    "key": "KON", "tasks": []});
  let response = server.request("POST", "/projects", Some(token), Some(body)).await;
  assert_eq!(response.status, 201);
  response.json()
}

pub(crate) async fn email_invite(server: &TestServer, token: &Credential, body: Value) -> InviteCreated {
  let response = server.request("POST", "/invites", Some(token), Some(body)).await;
  assert_eq!(response.status, 201);
  response.json()
}

#[tokio::test]
async fn s1c_a_cloud_invite_joins_once_locally_and_gives_only_its_project() {
  let (server, admin, cloud, audience) = linked().await;
  let info: ServerInfo = server.request("GET", "/server", None, None).await.json();
  assert_eq!(info.cloud.expect("linked").root_fingerprint, cloud.root.fingerprint());
  let kontur = project(&server, &admin.token, "kontur").await;
  let billing = project(&server, &admin.token, "billing").await;
  let invite = email_invite(
    &server,
    &admin.token,
    json!({"kind":"project","project":kontur.project.id,"role":"member","delivery":"cloud","email":"bob@example.com"}),
  )
  .await;
  assert!(invite.code.is_none());
  assert_eq!(invite.signed.expect("signed for the cloud").split('.').count(), 3);
  let invite_id = invite.invite.id.to_string();
  refused(
    &cloud_join(&server, &cloud.token(&bob(&audience, None), server.seconds()), None).await,
    403,
    ErrorCode::NotInvited,
  );
  let token = cloud.token(&bob(&audience, Some(&invite_id)), server.seconds());
  let joined = cloud_join(&server, &token, None).await;
  assert_eq!(joined.status, 201);
  let joined: Joined = joined.json();
  assert!(joined.user.cloud && !joined.user.admin);
  assert_eq!(joined.user.display_name, "Bob Koval");
  assert_eq!(joined.session.via, JoinedVia::Cloud);
  let membership = joined.joined.expect("project membership");
  assert_eq!(
    (membership.project, membership.role),
    (kontur.project.id, MemberRole::Member)
  );
  refused(
    &cloud_join(&server, &token, None).await,
    401,
    ErrorCode::InvalidIdentity,
  );
  let kontur_path = format!("/projects/{}", kontur.project.id);
  let billing_path = format!("/projects/{}", billing.project.id);
  assert_eq!(
    server
      .request("GET", &kontur_path, Some(&joined.token), None)
      .await
      .status,
    200
  );
  assert_eq!(
    server
      .request("GET", &billing_path, Some(&joined.token), None)
      .await
      .status,
    404
  );
  // A new computer of the same cloud account joins without an invite.
  let second = cloud_join(&server, &cloud.token(&bob(&audience, None), server.seconds()), None).await;
  assert_eq!(second.status, 201);
  assert_eq!(second.json::<Joined>().user.id, joined.user.id);
  let audit: i64 = server
    .database()
    .query_row(
      "SELECT COUNT(*) FROM audit_events WHERE action='joined' AND target LIKE 'cloud:%'",
      [],
      |row| row.get(0),
    )
    .expect("audited joins");
  assert_eq!(audit, 2);
  server.finish().await;
}

#[tokio::test]
async fn s1c_expired_wrong_audience_and_forged_identity_tokens_are_refused_offline() {
  let (server, admin, cloud, audience) = linked().await;
  let invite = email_invite(
    &server,
    &admin.token,
    json!({"kind":"server","delivery":"cloud","email":"bob@example.com"}),
  )
  .await;
  let invite_id = invite.invite.id.to_string();
  let now = server.seconds();
  let expired = cloud.token(&bob(&audience, Some(&invite_id)), now - 300 - 121);
  refused(
    &cloud_join(&server, &expired, None).await,
    401,
    ErrorCode::InvalidIdentity,
  );
  let elsewhere = cloud.token(&bob("another-server", Some(&invite_id)), now);
  refused(
    &cloud_join(&server, &elsewhere, None).await,
    401,
    ErrorCode::InvalidIdentity,
  );
  let stranger = Key::generate();
  let forged = TestCloud::token_by(&stranger, &bob(&audience, Some(&invite_id)), now);
  refused(
    &cloud_join(&server, &forged, None).await,
    401,
    ErrorCode::InvalidIdentity,
  );
  let genuine = cloud.token(&bob(&audience, Some(&invite_id)), now);
  let mut parts: Vec<String> = genuine.split('.').map(str::to_owned).collect();
  parts[1] = parts[1].replace('A', "B");
  refused(
    &cloud_join(&server, &parts.join("."), None).await,
    401,
    ErrorCode::InvalidIdentity,
  );
  assert_eq!(cloud_join(&server, &genuine, None).await.status, 201);
  server.finish().await;
}

#[tokio::test]
async fn s1c_key_sets_rotate_expire_and_arrive_with_a_joining_app() {
  let (server, admin, cloud, audience) = linked().await;
  let invite = email_invite(
    &server,
    &admin.token,
    json!({"kind":"server","delivery":"cloud","email":"bob@example.com"}),
  )
  .await;
  let invite_id = invite.invite.id.to_string();
  // The cloud rotates to a new signing key; an app carries the newer key set, which replaces the cached one.
  let newer = Key::generate();
  let rotated = TestCloud::key_set_by(&cloud.root, &[&newer], server.seconds() + 60);
  let old_key_token = cloud.token(&bob(&audience, Some(&invite_id)), server.seconds());
  refused(
    &cloud_join(&server, &old_key_token, Some(&rotated)).await,
    401,
    ErrorCode::InvalidIdentity,
  );
  let joined = cloud_join(
    &server,
    &TestCloud::token_by(&newer, &bob(&audience, Some(&invite_id)), server.seconds()),
    Some(&rotated),
  )
  .await;
  assert_eq!(joined.status, 201);
  // A key set signed by another root is ignored, not cached.
  let impostor = TestCloud::generate();
  let foreign = impostor.key_set(server.seconds() + 120);
  let token = TestCloud::token_by(&impostor.signing, &bob(&audience, None), server.seconds());
  refused(
    &cloud_join(&server, &token, Some(&foreign)).await,
    401,
    ErrorCode::InvalidIdentity,
  );
  // Without joining apps for 30 days the cached key set expires; cloud joins wait for a newer one.
  server.clock.advance(31 * DAY * 1000);
  let late = TestCloud::token_by(&newer, &bob(&audience, None), server.seconds());
  refused(&cloud_join(&server, &late, None).await, 409, ErrorCode::KeySetStale);
  assert_eq!(
    server
      .request("GET", "/auth/me", Some(&joined.json::<Joined>().token), None)
      .await
      .status,
    200
  );
  let fresh = TestCloud::key_set_by(&cloud.root, &[&newer], server.seconds());
  assert_eq!(cloud_join(&server, &late, Some(&fresh)).await.status, 201);
  server.finish().await;
}

#[tokio::test]
async fn s1c_tokens_keep_verifying_when_the_cloud_moves_to_another_url() {
  const MOVED: &str = "https://moved.cloud.test";
  let (server, admin, cloud, audience) = linked().await;
  let invite = email_invite(
    &server,
    &admin.token,
    json!({"kind":"server","delivery":"cloud","email":"bob@example.com"}),
  )
  .await;
  let invite_id = invite.invite.id.to_string();
  // Until an app brings a key set that names the moved cloud, its issuer is not the cached set's.
  let early = cloud.token_issued_by(MOVED, &bob(&audience, Some(&invite_id)), server.seconds());
  refused(
    &cloud_join(&server, &early, None).await,
    401,
    ErrorCode::InvalidIdentity,
  );
  let moved = TestCloud::key_set_named(&cloud.root, &[&cloud.signing], MOVED, server.seconds() + 60);
  let joined = cloud_join(&server, &early, Some(&moved)).await;
  assert_eq!(joined.status, 201);
  let former: Option<String> = server
    .database()
    .query_row("SELECT former_issuer FROM cloud_link WHERE id=1", [], |row| row.get(0))
    .expect("the link");
  assert_eq!(former.as_deref(), Some(crate::support::cloud::ISSUER));
  // A token issued just before the move still verifies, after the moved cloud's next key set too.
  let before = cloud.token(&bob(&audience, None), server.seconds());
  assert_eq!(cloud_join(&server, &before, None).await.status, 201);
  let weekly = TestCloud::key_set_named(&cloud.root, &[&cloud.signing], MOVED, server.seconds() + 120);
  let still = cloud.token(&bob(&audience, None), server.seconds());
  assert_eq!(cloud_join(&server, &still, Some(&weekly)).await.status, 201);
  let elsewhere = cloud.token_issued_by("https://elsewhere.test", &bob(&audience, None), server.seconds());
  refused(
    &cloud_join(&server, &elsewhere, None).await,
    401,
    ErrorCode::InvalidIdentity,
  );
  server.finish().await;
}

/// Bob, invited by email, joined through the cloud.
pub(crate) async fn bob_joined(server: &TestServer, admin: &Joined, cloud: &TestCloud, audience: &str) -> Joined {
  let body = json!({"kind":"server","delivery":"cloud","email":"bob@example.com"});
  let invite = email_invite(server, &admin.token, body).await;
  let invite_id = invite.invite.id.to_string();
  let token = cloud.token(&bob(audience, Some(&invite_id)), server.seconds());
  cloud_join(server, &token, None).await.json()
}

#[tokio::test]
async fn s1c_the_owner_turns_the_cloud_off_and_joined_devices_keep_working() {
  let (server, admin, cloud, audience) = linked().await;
  let joined = bob_joined(&server, &admin, &cloud, &audience).await;
  let body = json!({"accepting":false});
  let off = server
    .request("PATCH", "/server/cloud", Some(&admin.token), Some(body))
    .await;
  assert!(!off.json::<CloudLinkResponse>().cloud.accepting);
  assert!(
    !server
      .request("GET", "/server", None, None)
      .await
      .json::<ServerInfo>()
      .cloud
      .expect("linked")
      .accepting
  );
  let token = cloud.token(&bob(&audience, None), server.seconds());
  refused(&cloud_join(&server, &token, None).await, 403, ErrorCode::CloudOff);
  let me = server.request("GET", "/auth/me", Some(&joined.token), None).await;
  assert_eq!(me.status, 200);
  server.server.host_cloud(CloudCommand::On).await.expect("cloud on");
  assert_eq!(cloud_join(&server, &token, None).await.status, 201);
  server.finish().await;
}

#[tokio::test]
async fn s1c_the_owner_re_pins_the_cloud_root_and_unlinks() {
  let (server, admin, cloud, audience) = linked().await;
  bob_joined(&server, &admin, &cloud, &audience).await;
  // After a root compromise the owner pins a new root: the old root's key set goes, its tokens stop.
  let replacement = TestCloud::generate();
  let pin = CloudCommand::Pin(replacement.root.public());
  let pinned = server.server.host_cloud(pin).await.expect("pin");
  assert_eq!(
    pinned.root_fingerprint.as_deref(),
    Some(replacement.root.fingerprint().as_str())
  );
  let old_token = cloud.token(&bob(&audience, None), server.seconds());
  refused(
    &cloud_join(&server, &old_token, None).await,
    409,
    ErrorCode::KeySetStale,
  );
  let old_root_set = cloud.key_set(server.seconds());
  let old_token = cloud.token(&bob(&audience, None), server.seconds());
  refused(
    &cloud_join(&server, &old_token, Some(&old_root_set)).await,
    409,
    ErrorCode::KeySetStale,
  );
  let new_set = replacement.key_set(server.seconds());
  let token = replacement.token(&bob(&audience, None), server.seconds());
  assert_eq!(cloud_join(&server, &token, Some(&new_set)).await.status, 201);
  let body = json!({"unlink":true});
  let unlinked: CloudLinkResponse = server
    .request("PATCH", "/server/cloud", Some(&admin.token), Some(body))
    .await
    .json();
  assert!(unlinked.cloud.root_fingerprint.is_none());
  assert!(
    server
      .request("GET", "/server", None, None)
      .await
      .json::<ServerInfo>()
      .cloud
      .is_none()
  );
  let token = replacement.token(&bob(&audience, None), server.seconds());
  refused(&cloud_join(&server, &token, None).await, 403, ErrorCode::CloudOff);
  server.finish().await;
}

#[tokio::test]
async fn s1c_a_code_account_links_its_cloud_identity_and_an_account_invite_names_an_existing_account() {
  let (server, admin, cloud, audience) = linked().await;
  // The setup code shows no cloud identity: its account links one later, from a device of its own.
  let identity = Identity {
    audience: &audience,
    subject: "0191c7c0-0000-7000-8000-00000000a440",
    email: "alice@example.com",
    name: "Alice",
    invite: None,
  };
  let body = json!({"identity_token": cloud.token(&identity, server.seconds()), "device": "Alice's laptop"});
  let linked = server
    .request("POST", "/auth/cloud", Some(&admin.token), Some(body))
    .await;
  assert_eq!(linked.status, 200);
  assert!(linked.json::<UserResponse>().user.cloud);
  let again: Joined = cloud_join(&server, &cloud.token(&identity, server.seconds()), None)
    .await
    .json();
  assert_eq!(again.user.id, admin.user.id);
  // An invite code links no cloud identity: the account it makes links one from a device of its own.
  let carol = server.invited("carol", false).await;
  assert!(!carol.user.cloud);
  let identity = Identity {
    audience: &audience,
    subject: "0191c7c0-0000-7000-8000-00000000c440",
    email: "carol@example.com",
    name: "Carol",
    invite: None,
  };
  let body = json!({"identity_token": cloud.token(&identity, server.seconds()), "device": "Carol's laptop"});
  let linked = server
    .request("POST", "/auth/cloud", Some(&carol.token), Some(body))
    .await;
  assert_eq!(linked.status, 200);
  let again: Joined = cloud_join(&server, &cloud.token(&identity, server.seconds()), None)
    .await
    .json();
  assert_eq!(again.user.id, carol.user.id);
  // Dave lost every device; an account invite by email links his account to his cloud identity.
  let dave = server.invited("dave", false).await;
  let invite = email_invite(
    &server,
    &admin.token,
    json!({"kind":"account","user":dave.user.id,"delivery":"cloud","email":"dave@example.com"}),
  )
  .await;
  let invite_id = invite.invite.id.to_string();
  let identity = Identity {
    audience: &audience,
    subject: "0191c7c0-0000-7000-8000-00000000d440",
    email: "DAVE@example.com",
    name: "Dave",
    invite: Some(&invite_id),
  };
  let joined: Joined = cloud_join(&server, &cloud.token(&identity, server.seconds()), None)
    .await
    .json();
  assert_eq!(joined.user.id, dave.user.id);
  let unknown = Identity {
    audience: &audience,
    subject: "0191c7c0-0000-7000-8000-00000000e440",
    email: "eve@example.com",
    name: "Eve",
    invite: Some(&invite_id),
  };
  refused(
    &cloud_join(&server, &cloud.token(&unknown, server.seconds()), None).await,
    403,
    ErrorCode::NotInvited,
  );
  server.finish().await;
}
