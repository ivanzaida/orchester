//! The team server registry and invites in flight: a server is linked from the statement it signed, its admin gets
//! tokens for its admin, and invites it signed reach the invitee until it revokes them.

use orchester_api::{
  InviteKind, MemberRole,
  cloud::{
    CloudInvites, IdentityTokenIssued, KeySetResponse, ServerLinked, ServerResponse, ServersResponse, TokenPurpose,
  },
  identity::{PrivateKey, PublicKey, SigningKey, VerifiedKeySet},
  statements::{
    INVITE_REVOCATION_TYPE, InviteRevocation, InvitedProject, LINK_STATEMENT_TYPE, LinkStatement, SIGNED_INVITE_TYPE,
    SignedInvite,
  },
};
use orchester_cloud::{clock::Clock, mail::EmailPurpose};
use orchester_core::id::{InviteId, TeamServerId};
use serde_json::json;

use crate::support::{DAY, MINUTE, PASSWORD, TestCloud, remember};

/// A team server's own key and id, as it signs for the cloud.
pub(crate) struct TeamServer {
  id: TeamServerId,
  key: SigningKey,
}

impl TeamServer {
  pub(crate) fn generate() -> Self {
    Self {
      id: TeamServerId::generate(),
      key: SigningKey::new(&PrivateKey::generate().expect("server key")).expect("signing key"),
    }
  }

  fn statement(&self, signer: &SigningKey, issued_at_ms: i64) -> String {
    let statement = LinkStatement {
      server: self.id,
      name: "Kontur team".into(),
      address: "https://orchester.kontur.test".into(),
      public_key: self.key.public_key().encoded(),
      fingerprint: Some("ab".repeat(32)),
      issued_at: issued_at_ms / 1000,
    };
    signer
      .sign_statement(LINK_STATEMENT_TYPE, &statement)
      .expect("statement")
  }

  pub(crate) fn invite(&self, id: InviteId, email: &str, now: i64) -> String {
    let invite = SignedInvite {
      id,
      server: self.id,
      kind: InviteKind::Project,
      email: email.into(),
      project: Some(InvitedProject {
        address: "https://orchester.kontur.test/kontur".into(),
        name: "Kontur".into(),
      }),
      role: Some(MemberRole::Member),
      inviter: "Ivan Zaida".into(),
      expires_at: now + 7 * DAY,
      issued_at: now,
    };
    self.key.sign_statement(SIGNED_INVITE_TYPE, &invite).expect("invite")
  }

  fn revocation(&self, signer: &SigningKey, id: InviteId, now: i64) -> String {
    let revocation = InviteRevocation {
      id,
      server: self.id,
      issued_at: now,
    };
    signer
      .sign_statement(INVITE_REVOCATION_TYPE, &revocation)
      .expect("revocation")
  }
}

pub(crate) async fn signed_in(cloud: &TestCloud, email: &str) -> String {
  cloud.register(email).await;
  cloud.sign_in(email, PASSWORD, "Edge").await.token.expose().to_owned()
}

pub(crate) async fn link(cloud: &TestCloud, device: &str, server: &TeamServer) -> ServerLinked {
  let body = json!({"link": server.statement(&server.key, cloud.clock.now_ms())});
  let response = cloud.request("POST", "/servers", Some(device), Some(body)).await;
  assert_eq!(response.status, 201, "{}", response.error());
  let linked: ServerLinked = response.json();
  remember(linked.identity_token.expose());
  linked
}

pub async fn servers_link_from_the_statement_they_signed() {
  let cloud = TestCloud::start().await;
  cloud.publish_keys().await;
  let owner = signed_in(&cloud, "ivan@example.test").await;
  let server = TeamServer::generate();
  let forged = json!({"link": server.statement(&TeamServer::generate().key, cloud.clock.now_ms())});
  let refused = cloud.request("POST", "/servers", Some(&owner), Some(forged)).await;
  assert_eq!((refused.status, refused.error()), (422, "invalid".into()));
  let stale = json!({"link": server.statement(&server.key, cloud.clock.now_ms() - 11 * MINUTE)});
  let refused = cloud.request("POST", "/servers", Some(&owner), Some(stale)).await;
  assert_eq!((refused.status, refused.error()), (422, "invalid".into()));
  let linked = link(&cloud, &owner, &server).await;
  assert_eq!(linked.server.server.id, server.id);
  assert_eq!(linked.server.check_code.as_deref(), Some("NENT-QAXB-NENT-QAXB"));
  let root = PublicKey::parse(&linked.root_key).expect("root key");
  assert_eq!(root, cloud.root.public_key());
  let set = VerifiedKeySet::verify(&linked.key_set, &root, cloud.clock.now_ms() / 1000).expect("key set");
  let claims = set
    .verify_token(
      linked.identity_token.expose(),
      &server.id.to_string(),
      cloud.clock.now_ms() / 1000,
    )
    .expect("a token for the server")
    .claims;
  assert_eq!(claims.purpose, Some(TokenPurpose::Link));
  let listed: ServersResponse = cloud.request("GET", "/servers", Some(&owner), None).await.json();
  assert_eq!(listed.servers.len(), 1);
  assert!(listed.servers[0].owner);
  let other = signed_in(&cloud, "mallory@example.test").await;
  let path = format!("/servers/{}", server.id);
  let shown: ServerResponse = cloud.request("GET", &path, Some(&other), None).await.json();
  assert_eq!(shown.server.server.name, "Kontur team");
  let taken = json!({"link": server.statement(&server.key, cloud.clock.now_ms())});
  let refused = cloud.request("POST", "/servers", Some(&other), Some(taken)).await;
  assert_eq!((refused.status, refused.error()), (409, "conflict".into()));
  let again = link(&cloud, &owner, &server).await;
  assert_eq!(again.server.server.id, server.id, "its owner links it again");
  cloud.finish().await;
}

pub async fn admin_tokens_are_issued_only_for_linked_servers() {
  let cloud = TestCloud::start().await;
  cloud.publish_keys().await;
  let owner = signed_in(&cloud, "ivan@example.test").await;
  let server = TeamServer::generate();
  let admin = json!({"audience": server.id, "purpose": "admin"});
  let unknown = cloud
    .request("POST", "/identity-tokens", Some(&owner), Some(admin.clone()))
    .await;
  assert_eq!((unknown.status, unknown.error()), (404, "not_found".into()));
  link(&cloud, &owner, &server).await;
  let issued = cloud
    .request("POST", "/identity-tokens", Some(&owner), Some(admin))
    .await;
  assert_eq!(issued.status, 201);
  let issued: IdentityTokenIssued = issued.json();
  remember(issued.identity_token.expose());
  let keys: KeySetResponse = cloud.request("GET", "/keys", None, None).await.json();
  let set = VerifiedKeySet::verify(&keys.key_set, &cloud.root.public_key(), cloud.clock.now_ms() / 1000).expect("keys");
  let claims = set
    .verify_token(
      issued.identity_token.expose(),
      &server.id.to_string(),
      cloud.clock.now_ms() / 1000,
    )
    .expect("verifies")
    .claims;
  assert_eq!(claims.purpose, Some(TokenPurpose::Admin));
  let link_purpose = json!({"audience": server.id, "purpose": "link"});
  let refused = cloud
    .request("POST", "/identity-tokens", Some(&owner), Some(link_purpose))
    .await;
  assert_eq!((refused.status, refused.error()), (422, "invalid".into()));
  cloud.finish().await;
}

pub async fn invites_reach_the_invitee_once_and_end_when_revoked() {
  let cloud = TestCloud::start().await;
  cloud.publish_keys().await;
  let owner = signed_in(&cloud, "ivan@example.test").await;
  let dana = signed_in(&cloud, "dana@example.test").await;
  let server = TeamServer::generate();
  let now = cloud.clock.now_ms();
  let id = InviteId::generate();
  let unlinked = cloud
    .request(
      "POST",
      "/invites",
      None,
      Some(json!({"invite": server.invite(id, "dana@example.test", now)})),
    )
    .await;
  assert_eq!((unlinked.status, unlinked.error()), (422, "invalid".into()));
  link(&cloud, &owner, &server).await;
  deliver_twice(&cloud, &server, id, now).await;
  listed_to_the_invitee_only(&cloud, &dana, &owner).await;
  newcomers_are_asked_to_sign_up(&cloud, &server, now).await;
  revoke(&cloud, &server, id, now).await;
  let listed: CloudInvites = cloud.request("GET", "/invites", Some(&dana), None).await.json();
  assert!(listed.invites.is_empty());
  cloud.clock.advance(8 * DAY);
  cloud.cloud.sweep().await.expect("sweep");
  let left: i64 = crate::support::postgres()
    .database(&cloud.database)
    .await
    .query_one("SELECT count(*) FROM invites", &[])
    .await
    .expect("count")
    .get(0);
  assert_eq!(left, 0, "expired invites are deleted");
  cloud.finish().await;
}

/// The server and the inviter's browser both deliver it: one invite, one email, in the cloud's own words.
async fn deliver_twice(cloud: &TestCloud, server: &TeamServer, id: InviteId, now: i64) {
  let body = json!({"invite": server.invite(id, "Dana@Example.test", now)});
  assert_eq!(
    cloud.request("POST", "/invites", None, Some(body.clone())).await.status,
    202
  );
  assert_eq!(cloud.request("POST", "/invites", None, Some(body)).await.status, 202);
  let email = cloud.mail.nth("dana@example.test", 2).await;
  assert_eq!(email.purpose, EmailPurpose::Invite);
  assert!(
    !email.body.contains("Kontur") && !email.body.contains("Ivan"),
    "no server-chosen words"
  );
  assert_eq!(
    cloud.mail.count("dana@example.test"),
    2,
    "the confirmation and one invite"
  );
}

async fn listed_to_the_invitee_only(cloud: &TestCloud, invitee: &str, other: &str) {
  let listed: CloudInvites = cloud.request("GET", "/invites", Some(invitee), None).await.json();
  assert_eq!(listed.invites.len(), 1);
  let invite = &listed.invites[0];
  assert_eq!(
    (invite.server.name.as_str(), invite.inviter_display_name.as_str()),
    ("Kontur team", "Ivan Zaida")
  );
  assert_eq!(
    invite.project.as_ref().map(|project| project.name.as_str()),
    Some("Kontur")
  );
  let theirs: CloudInvites = cloud.request("GET", "/invites", Some(other), None).await.json();
  assert!(theirs.invites.is_empty());
}

async fn newcomers_are_asked_to_sign_up(cloud: &TestCloud, server: &TeamServer, now: i64) {
  let id = InviteId::generate();
  let body = json!({"invite": server.invite(id, "new@example.test", now)});
  assert_eq!(cloud.request("POST", "/invites", None, Some(body)).await.status, 202);
  let email = cloud.mail.nth("new@example.test", 1).await;
  assert!(email.body.contains(&format!("/signup?invite={id}")));
}

async fn revoke(cloud: &TestCloud, server: &TeamServer, id: InviteId, now: i64) {
  let path = format!("/invites/{id}");
  let forged = json!({"revocation": server.revocation(&TeamServer::generate().key, id, now)});
  let refused = cloud.request("DELETE", &path, None, Some(forged)).await;
  assert_eq!((refused.status, refused.error()), (422, "invalid".into()));
  let revoked = json!({"revocation": server.revocation(&server.key, id, now)});
  assert_eq!(cloud.request("DELETE", &path, None, Some(revoked)).await.status, 204);
}

/// The invitee declines an invite: it is gone from their list, declining it again or someone else's is `404`.
pub async fn invites_are_declined_by_the_invitee() {
  let cloud = TestCloud::start().await;
  cloud.publish_keys().await;
  let owner = signed_in(&cloud, "ivan@example.test").await;
  let dana = signed_in(&cloud, "dana@example.test").await;
  let server = TeamServer::generate();
  link(&cloud, &owner, &server).await;
  let id = InviteId::generate();
  let body = json!({"invite": server.invite(id, "dana@example.test", cloud.clock.now_ms())});
  assert_eq!(cloud.request("POST", "/invites", None, Some(body)).await.status, 202);
  let decline = format!("/invites/{id}/decline");
  let stranger = cloud.request("POST", &decline, Some(&owner), None).await;
  assert_eq!(stranger.status, 404, "only the invitee declines");
  assert_eq!(cloud.request("POST", &decline, Some(&dana), None).await.status, 204);
  let listed: CloudInvites = cloud.request("GET", "/invites", Some(&dana), None).await.json();
  assert!(listed.invites.is_empty());
  assert_eq!(cloud.request("POST", &decline, Some(&dana), None).await.status, 404);
  let malformed = cloud
    .request("POST", "/invites/not-an-id/decline", Some(&dana), None)
    .await;
  assert_eq!(malformed.status, 404);
  let anonymous = cloud.request("POST", &decline, None, None).await;
  assert_eq!(anonymous.status, 401);
  cloud.finish().await;
}
