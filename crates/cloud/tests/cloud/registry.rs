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
  pub(crate) fn id(&self) -> TeamServerId {
    self.id
  }

  /// A server as a team server makes itself: its id is its key's.
  pub(crate) fn generate() -> Self {
    let key = SigningKey::new(&PrivateKey::generate().expect("server key")).expect("signing key");
    Self {
      id: key.public_key().server_id(),
      key,
    }
  }

  /// A fresh statement of this server's, signed with its own key.
  fn statement(&self, issued_at_ms: i64) -> String {
    self.statement_by(&self.key, issued_at_ms)
  }

  /// A statement naming this server's id, signed by `signer`, which names its own key: what someone squatting the id
  /// with a key of their own would send.
  fn statement_by(&self, signer: &SigningKey, issued_at_ms: i64) -> String {
    let statement = LinkStatement {
      server: self.id,
      nonce: InviteId::generate().as_uuid().simple().to_string(),
      name: "Kontur team".into(),
      address: "https://orchester.kontur.test".into(),
      public_key: signer.public_key().encoded(),
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
  link_with(cloud, device, &server.statement(cloud.clock.now_ms())).await
}

async fn link_with(cloud: &TestCloud, device: &str, statement: &str) -> ServerLinked {
  let response = cloud
    .request("POST", "/servers", Some(device), Some(json!({"link": statement})))
    .await;
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
  let stale = json!({"link": server.statement(cloud.clock.now_ms() - 11 * MINUTE)});
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
  cloud.finish().await;
}

/// Security review 1, B1: a server's id is its own key's, so nobody registers another server's id with a key of
/// their own.
pub async fn a_server_id_is_its_own_keys() {
  let cloud = TestCloud::start().await;
  cloud.publish_keys().await;
  let owner = signed_in(&cloud, "ivan@example.test").await;
  let mallory = signed_in(&cloud, "mallory@example.test").await;
  let server = TeamServer::generate();
  let attacker = TeamServer::generate();
  for device in [&mallory, &owner] {
    let squat = json!({"link": server.statement_by(&attacker.key, cloud.clock.now_ms())});
    let refused = cloud.request("POST", "/servers", Some(device), Some(squat)).await;
    assert_eq!(
      (refused.status, refused.error()),
      (422, "invalid".into()),
      "before the server linked"
    );
  }
  link(&cloud, &owner, &server).await;
  let squat = json!({"link": server.statement_by(&attacker.key, cloud.clock.now_ms())});
  let refused = cloud.request("POST", "/servers", Some(&mallory), Some(squat)).await;
  assert_eq!(
    (refused.status, refused.error()),
    (422, "invalid".into()),
    "after it linked"
  );
  let own = link(&cloud, &mallory, &attacker).await;
  assert_ne!(
    own.server.server.id, server.id,
    "a key of one's own links a server of one's own"
  );
  let shown: ServerResponse = cloud
    .request("GET", &format!("/servers/{}", server.id), Some(&mallory), None)
    .await
    .json();
  assert_eq!(shown.server.server.address, "https://orchester.kontur.test");
  cloud.finish().await;
}

/// Security review 1, B1: a statement works once; a newer one signed by the server's key takes the server over, an
/// older one does not.
pub async fn a_statement_works_once_and_a_newer_one_takes_the_server_back() {
  let cloud = TestCloud::start().await;
  cloud.publish_keys().await;
  let owner = signed_in(&cloud, "ivan@example.test").await;
  let other = signed_in(&cloud, "alice@example.test").await;
  let server = TeamServer::generate();
  let older = server.statement(cloud.clock.now_ms());
  cloud.clock.advance(2000);
  let first = server.statement(cloud.clock.now_ms());
  link_with(&cloud, &other, &first).await;
  let replayed = cloud
    .request("POST", "/servers", Some(&owner), Some(json!({"link": first})))
    .await;
  assert_eq!(
    (replayed.status, replayed.error()),
    (409, "conflict".into()),
    "a replay"
  );
  let stale = cloud
    .request("POST", "/servers", Some(&owner), Some(json!({"link": older})))
    .await;
  assert_eq!(
    (stale.status, stale.error()),
    (409, "conflict".into()),
    "an older statement"
  );
  cloud.clock.advance(2000);
  link_with(&cloud, &owner, &server.statement(cloud.clock.now_ms())).await;
  let owned: ServersResponse = cloud.request("GET", "/servers", Some(&owner), None).await.json();
  assert!(
    owned
      .servers
      .iter()
      .any(|entry| entry.server.id == server.id && entry.owner)
  );
  let former: ServersResponse = cloud.request("GET", "/servers", Some(&other), None).await.json();
  assert!(
    former.servers.iter().all(|entry| !entry.owner),
    "the former owner no longer owns it"
  );
  cloud.finish().await;
}

/// Security review 1, M2: one account owns at most five linked servers.
pub async fn an_account_owns_at_most_five_servers() {
  let cloud = TestCloud::start().await;
  cloud.publish_keys().await;
  let owner = signed_in(&cloud, "ivan@example.test").await;
  let servers: Vec<TeamServer> = (0..6).map(|_| TeamServer::generate()).collect();
  for server in &servers[..5] {
    link(&cloud, &owner, server).await;
  }
  let sixth = json!({"link": servers[5].statement(cloud.clock.now_ms())});
  let refused = cloud.request("POST", "/servers", Some(&owner), Some(sixth)).await;
  assert_eq!((refused.status, refused.error()), (409, "too_many_servers".into()));
  cloud.clock.advance(2000);
  link(&cloud, &owner, &servers[0]).await;
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
  declined_stays_declined(&cloud, &dana, (&server, id)).await;
  let malformed = cloud
    .request("POST", "/invites/not-an-id/decline", Some(&dana), None)
    .await;
  assert_eq!(malformed.status, 404);
  let anonymous = cloud.request("POST", &decline, None, None).await;
  assert_eq!(anonymous.status, 401);
  cloud.finish().await;
}

/// N1 security review 1, minor 1: the answer stays with the invite, so delivering it again brings nothing back, and a
/// declined invite issues no token.
async fn declined_stays_declined(cloud: &TestCloud, dana: &str, (server, id): (&TeamServer, InviteId)) {
  let again = json!({"invite": server.invite(id, "dana@example.test", cloud.clock.now_ms())});
  assert_eq!(cloud.request("POST", "/invites", None, Some(again)).await.status, 202);
  let listed: CloudInvites = cloud.request("GET", "/invites", Some(dana), None).await.json();
  assert!(listed.invites.is_empty(), "declined stays declined");
  let accept = json!({"audience": server.id, "invite": id});
  let token = cloud
    .request("POST", "/identity-tokens", Some(dana), Some(accept))
    .await;
  assert_eq!(token.status, 404, "a declined invite issues no token");
}

async fn deliver(cloud: &TestCloud, invite: &str, from: &str) -> crate::support::http::Response {
  cloud
    .request_headers(
      "POST",
      "/invites",
      None,
      Some(json!({"invite": invite})),
      &[("X-Forwarded-For", from)],
    )
    .await
}

/// Security review 1, minor 1: a revoked invite stays revoked: delivering it again is refused until it would have
/// expired. The answer to a delivery names the server as the cloud registered it.
pub async fn a_revoked_invite_cannot_be_delivered_again() {
  let cloud = TestCloud::start().await;
  cloud.publish_keys().await;
  let owner = signed_in(&cloud, "ivan@example.test").await;
  let server = TeamServer::generate();
  link(&cloud, &owner, &server).await;
  let now = cloud.clock.now_ms();
  let id = InviteId::generate();
  let invite = json!({"invite": server.invite(id, "dana@example.test", now)});
  let first = cloud.request("POST", "/invites", None, Some(invite.clone())).await;
  assert_eq!(first.status, 202);
  let answer: serde_json::Value = first.json();
  assert_eq!(answer["server"]["address"], json!("https://orchester.kontur.test"));
  let revocation = json!({"revocation": server.revocation(&server.key, id, now)});
  let path = format!("/invites/{id}");
  assert_eq!(cloud.request("DELETE", &path, None, Some(revocation)).await.status, 204);
  let again = cloud.request("POST", "/invites", None, Some(invite.clone())).await;
  assert_eq!((again.status, again.error()), (422, "invalid".into()));
  cloud.clock.advance(8 * DAY);
  cloud.cloud.sweep().await.expect("sweep");
  cloud.finish().await;
}

/// Security review 1, M2: invite emails have buckets of their own, one a day per server and address and three a day
/// per address, so invites neither spam an address nor use up its confirmation and reset emails.
pub async fn invite_emails_have_their_own_buckets() {
  let cloud = TestCloud::start_with(vec!["127.0.0.1".parse().expect("proxy")]).await;
  cloud.publish_keys().await;
  let owner = signed_in(&cloud, "ivan@example.test").await;
  let dana = signed_in(&cloud, "dana@example.test").await;
  let servers: Vec<TeamServer> = (0..5).map(|_| TeamServer::generate()).collect();
  for server in &servers {
    link(&cloud, &owner, server).await;
  }
  let now = cloud.clock.now_ms();
  for server in &servers {
    for _ in 0..2 {
      let invite = server.invite(InviteId::generate(), "dana@example.test", now);
      assert_eq!(deliver(&cloud, &invite, "203.0.113.20").await.status, 202);
    }
  }
  let listed: CloudInvites = cloud.request("GET", "/invites", Some(&dana), None).await.json();
  assert_eq!(listed.invites.len(), 10, "every invite is kept and listed");
  tokio::time::sleep(std::time::Duration::from_millis(300)).await;
  assert_eq!(
    cloud.mail.count("dana@example.test"),
    1 + 3,
    "the confirmation and three invite emails"
  );
  let reset = cloud
    .request("POST", "/auth/reset", None, Some(json!({"email": "dana@example.test"})))
    .await;
  assert_eq!(reset.status, 202, "invites did not use up the address's own emails");
  assert_eq!(
    cloud.mail.nth("dana@example.test", 5).await.purpose,
    EmailPurpose::Reset
  );
  cloud.finish().await;
}

/// Security review 1, M2: `POST /invites` is limited per client address, and the servers one account owns send at
/// most 100 invites a day through the cloud.
pub async fn invite_deliveries_are_limited_per_address_and_per_owner() {
  let cloud = TestCloud::start_with(vec!["127.0.0.1".parse().expect("proxy")]).await;
  cloud.publish_keys().await;
  for _ in 0..60 {
    assert_eq!(deliver(&cloud, "not.an.invite", "203.0.113.30").await.status, 422);
  }
  let limited = deliver(&cloud, "not.an.invite", "203.0.113.30").await;
  assert_eq!((limited.status, limited.error()), (429, "rate_limited".into()));
  assert_eq!(
    deliver(&cloud, "not.an.invite", "203.0.113.31").await.status,
    422,
    "another address is not"
  );
  let owner = signed_in(&cloud, "ivan@example.test").await;
  let servers: Vec<TeamServer> = (0..2).map(|_| TeamServer::generate()).collect();
  for server in &servers {
    link(&cloud, &owner, server).await;
  }
  let now = cloud.clock.now_ms();
  let ids: Vec<InviteId> = (0..100).map(|_| InviteId::generate()).collect();
  for (index, id) in ids.iter().enumerate() {
    let invite = servers[index % 2].invite(*id, &format!("person{index}@example.test"), now);
    let from = format!("198.51.100.{}", index / 50);
    assert_eq!(deliver(&cloud, &invite, &from).await.status, 202, "invite {index}");
  }
  let over = servers[0].invite(InviteId::generate(), "one-more@example.test", now);
  let refused = deliver(&cloud, &over, "198.51.100.9").await;
  assert_eq!((refused.status, refused.error()), (429, "rate_limited".into()));
  // Revoking invites gives none of the day's back.
  for (index, id) in ids.iter().enumerate().take(10) {
    revoke(&cloud, &servers[index % 2], *id, now).await;
  }
  let after = servers[1].invite(InviteId::generate(), "after-revoking@example.test", now);
  assert_eq!(deliver(&cloud, &after, "198.51.100.9").await.status, 429);
  cloud.finish().await;
}

/// Security review 2, 2: an invite id is one server's own. Another server's revocation of the same id neither blocks
/// this server's invite nor takes the place of this server's own revocation.
pub async fn invite_ids_are_one_servers_own() {
  let cloud = TestCloud::start().await;
  cloud.publish_keys().await;
  let owner = signed_in(&cloud, "ivan@example.test").await;
  let (kontur, other) = (TeamServer::generate(), TeamServer::generate());
  link(&cloud, &owner, &kontur).await;
  link(&cloud, &owner, &other).await;
  let now = cloud.clock.now_ms();
  let id = InviteId::generate();
  revoke(&cloud, &other, id, now).await;
  let invite = json!({"invite": kontur.invite(id, "dana@example.test", now)});
  let delivered = cloud.request("POST", "/invites", None, Some(invite.clone())).await;
  assert_eq!(
    delivered.status, 202,
    "another server's revocation of the id is not this server's"
  );
  revoke(&cloud, &kontur, id, now).await;
  let again = cloud.request("POST", "/invites", None, Some(invite)).await;
  assert_eq!(
    (again.status, again.error()),
    (422, "invalid".into()),
    "this server's own revocation holds"
  );
  cloud.finish().await;
}
