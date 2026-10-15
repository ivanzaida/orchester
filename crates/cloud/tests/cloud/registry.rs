use orchester_api::{
  InviteKind, MemberRole,
  cloud::{
    AccountServer, CloudInvite, InviteAccepted, InviteStatement, InvitesResponse, KeySetResponse, LinkStatement,
    ServerResponse, ServersResponse,
  },
  identity::VerifiedKeySet,
};
use orchester_cloud::{clock::Clock, mail::EmailPurpose};
use orchester_core::id::InviteId;
use serde_json::json;

use crate::support::{BASE_URL, DAY, PASSWORD, Response, TeamServer, TestCloud, postgres, remember, team::FINGERPRINT};

async fn owner(cloud: &TestCloud, email: &str) -> String {
  cloud.register(email).await;
  cloud
    .sign_in(email, PASSWORD, "Owner's laptop")
    .await
    .token
    .expose()
    .to_owned()
}

async fn post_invite(cloud: &TestCloud, signed: &str) -> Response {
  cloud
    .request("POST", "/invites", None, Some(json!({"invite": signed})))
    .await
}

async fn invites(cloud: &TestCloud, token: &str) -> InvitesResponse {
  let response = cloud.request("GET", "/invites", Some(token), None).await;
  assert_eq!(response.status, 200);
  response.json()
}

async fn servers(cloud: &TestCloud, token: &str) -> Vec<AccountServer> {
  let response = cloud.request("GET", "/servers", Some(token), None).await;
  assert_eq!(response.status, 200);
  response.json::<ServersResponse>().servers
}

/// The owner links a server with its signed statement; the server id then stays with that owner and that key.
pub async fn a_server_is_linked_from_its_signed_statement() {
  let cloud = TestCloud::start().await;
  let token = owner(&cloud, "owner@example.test").await;
  let team = TeamServer::generate();
  let linked = cloud.link_server(&token, &team).await;
  assert_eq!(linked.status, 201);
  let server = linked.json::<ServerResponse>().server;
  assert_eq!((server.server.id, server.owner), (team.id, true));
  assert_eq!(server.server.address, team.address);
  assert_eq!(server.server.fingerprint.as_deref(), Some(FINGERPRINT));
  assert_eq!(servers(&cloud, &token).await, vec![server]);
  assert_eq!(
    cloud.link_server(&token, &team).await.status,
    201,
    "linking again updates"
  );
  let other = owner(&cloud, "other@example.test").await;
  let taken = cloud.link_server(&other, &team).await;
  assert_eq!((taken.status, taken.error()), (409, "conflict".into()));
  let impostor = TeamServer::with_id(team.id);
  let rekeyed = cloud.link_server(&token, &impostor).await;
  assert_eq!((rekeyed.status, rekeyed.error()), (409, "conflict".into()));
  assert!(servers(&cloud, &other).await.is_empty());
  refused_links(&cloud, &token, &team).await;
  let anonymous = cloud
    .request("POST", "/servers", None, Some(json!({"link": team.link("Kontur", 0)})))
    .await;
  assert_eq!(anonymous.status, 401);
  cloud.finish().await;
}

/// A stale statement, a forged signature, another statement type and malformed fields answer `422 invalid`.
async fn refused_links(cloud: &TestCloud, token: &str, team: &TeamServer) {
  let now = cloud.clock.now_ms();
  let stale = team.link("Kontur team", now - 2 * DAY);
  let fresh = team.link("Kontur team", now);
  let (body, signature) = fresh.rsplit_once('.').expect("a JWS");
  let flipped = if signature.starts_with('A') { 'B' } else { 'A' };
  let forged = format!("{body}.{flipped}{}", &signature[1..]);
  let statement = |address: &str, fingerprint: &str| LinkStatement {
    server: team.id,
    name: "Kontur team".into(),
    address: address.into(),
    public_key: team.public_key(),
    fingerprint: Some(fingerprint.into()),
    issued_at: now / 1000,
  };
  let bad_address = team.sign_link(&statement("ftp://team.example.test", FINGERPRINT));
  let trailing_slash = team.sign_link(&statement("https://team.example.test/", FINGERPRINT));
  let bad_fingerprint = team.sign_link(&statement("https://team.example.test", "AB"));
  let an_invite = team.sign_invite(&team.project_invite("x@example.test", now));
  for link in [
    stale,
    forged,
    bad_address,
    trailing_slash,
    bad_fingerprint,
    an_invite,
    "not a statement".into(),
  ] {
    let answer = cloud
      .request("POST", "/servers", Some(token), Some(json!({"link": link})))
      .await;
    assert_eq!((answer.status, answer.error()), (422, "invalid".into()));
  }
}

/// An invite its registered server signed reaches the invitee's account, with one email either way; carrying it
/// twice changes nothing.
pub async fn invites_are_routed_to_registered_and_new_emails() {
  let cloud = TestCloud::start().await;
  let token = owner(&cloud, "owner@example.test").await;
  let team = TeamServer::generate();
  assert_eq!(cloud.link_server(&token, &team).await.status, 201);
  let invitee = "bob@example.test";
  cloud.register(invitee).await;
  let bob = cloud.sign_in(invitee, PASSWORD, "Bob's laptop").await;
  let invite = team.project_invite(invitee, cloud.clock.now_ms());
  let signed = team.sign_invite(&invite);
  assert_eq!(post_invite(&cloud, &signed).await.status, 202);
  assert_eq!(post_invite(&cloud, &signed).await.status, 202, "the same invite again");
  let mail = cloud.mail.nth(invitee, 2).await;
  assert_eq!(mail.purpose, EmailPurpose::Invite);
  assert!(
    mail
      .body
      .contains("Tomas Berg invited you to the project Kontur on Kontur team")
  );
  assert!(mail.body.contains(&format!("{BASE_URL}/account")));
  let listed = invites(&cloud, bob.token.expose()).await.invites;
  assert_eq!(listed.len(), 1);
  the_listed_invite_is_the_signed_one(&listed[0], &invite, &team);
  assert!(
    invites(&cloud, &token).await.invites.is_empty(),
    "only the invitee sees it"
  );
  tokio::time::sleep(std::time::Duration::from_millis(300)).await;
  assert_eq!(cloud.mail.count(invitee), 2, "one email for the invite");

  an_invite_waits_for_a_new_account(&cloud, &team).await;
  cloud.finish().await;
}

fn the_listed_invite_is_the_signed_one(open: &CloudInvite, invite: &InviteStatement, team: &TeamServer) {
  assert_eq!(
    (open.id, open.kind, open.role, open.admin),
    (invite.id, InviteKind::Project, Some(MemberRole::Member), false)
  );
  assert_eq!((open.server.id, open.server.name.as_str()), (team.id, "Kontur team"));
  assert_eq!(
    open.project.as_ref().map(|project| project.name.as_str()),
    Some("Kontur")
  );
  assert_eq!(open.inviter_display_name, "Tomas Berg");
  assert_eq!(open.expires_at, invite.expires_at);
}

/// An email with no account is asked to sign up, and the invite waits for its account.
async fn an_invite_waits_for_a_new_account(cloud: &TestCloud, team: &TeamServer) {
  let newcomer = "carol@example.test";
  let mut later = team.project_invite(newcomer, cloud.clock.now_ms());
  later.kind = InviteKind::Server;
  (later.project, later.role, later.admin) = (None, None, true);
  assert_eq!(post_invite(cloud, &team.sign_invite(&later)).await.status, 202);
  let mail = cloud.mail.nth(newcomer, 1).await;
  assert!(
    mail
      .body
      .contains("Tomas Berg invited you to the team server Kontur team")
  );
  assert!(mail.body.contains(&format!("{BASE_URL}/signup")));
  cloud.register(newcomer).await;
  let carol = cloud.sign_in(newcomer, PASSWORD, "Carol's laptop").await;
  let waiting = invites(cloud, carol.token.expose()).await.invites;
  assert_eq!(
    (waiting.len(), waiting[0].kind, waiting[0].admin),
    (1, InviteKind::Server, true)
  );
}

/// Only an invite the registered server signed, within its times and in its shape, is taken.
pub async fn forged_or_malformed_invites_are_refused() {
  let cloud = TestCloud::start().await;
  let token = owner(&cloud, "owner@example.test").await;
  let team = TeamServer::generate();
  assert_eq!(cloud.link_server(&token, &team).await.status, 201);
  let now = cloud.clock.now_ms();
  let email = "dave@example.test";
  let unregistered = TeamServer::generate();
  let impostor = TeamServer::with_id(team.id);
  let mut expired = team.project_invite(email, now - 8 * DAY);
  expired.expires_at = now - 1;
  let mut too_long = team.project_invite(email, now);
  too_long.expires_at = now + 30 * DAY;
  let mut no_role = team.project_invite(email, now);
  no_role.role = None;
  let mut admin_project = team.project_invite(email, now);
  admin_project.admin = true;
  let mut bad_email = team.project_invite("not an email", now);
  bad_email.email = "not an email".into();
  let refused = [
    unregistered.sign_invite(&unregistered.project_invite(email, now)),
    impostor.sign_invite(&team.project_invite(email, now)),
    team.sign_invite(&expired),
    team.sign_invite(&too_long),
    team.sign_invite(&no_role),
    team.sign_invite(&admin_project),
    team.sign_invite(&bad_email),
    team.link("not an invite", now),
  ];
  for signed in refused {
    let answer = post_invite(&cloud, &signed).await;
    assert_eq!((answer.status, answer.error()), (422, "invalid".into()));
  }
  assert_eq!(cloud.mail.count(email), 0);
  cloud.finish().await;
}

/// Declining deletes the invite; accepting gives the server and a token for it that names the invite, and lists the
/// server among the account's. Without keys nothing changes.
pub async fn invites_are_declined_and_accepted() {
  let cloud = TestCloud::start().await;
  let token = owner(&cloud, "owner@example.test").await;
  let team = TeamServer::generate();
  assert_eq!(cloud.link_server(&token, &team).await.status, 201);
  let invitee = "erin@example.test";
  cloud.register(invitee).await;
  let erin = cloud.sign_in(invitee, PASSWORD, "Erin's laptop").await;
  let erin = erin.token.expose();
  let first = team.project_invite(invitee, cloud.clock.now_ms());
  let second = team.project_invite(invitee, cloud.clock.now_ms());
  for invite in [&first, &second] {
    assert_eq!(post_invite(&cloud, &team.sign_invite(invite)).await.status, 202);
  }
  only_the_invitee_declines(&cloud, &token, erin, first.id).await;
  accept_once_keys_are_published(&cloud, erin, &team, &second).await;
  let joined = servers(&cloud, erin).await;
  assert_eq!(
    (joined.len(), joined[0].server.id, joined[0].owner),
    (1, team.id, false)
  );
  cloud.finish().await;
}

/// Without a published key set accepting changes nothing; with one it ends the invite and gives a token naming it.
async fn accept_once_keys_are_published(cloud: &TestCloud, invitee: &str, team: &TeamServer, invite: &InviteStatement) {
  let accept = format!("/invites/{}/accept", invite.id);
  let no_keys = cloud.request("POST", &accept, Some(invitee), None).await;
  assert_eq!((no_keys.status, no_keys.error()), (503, "keys_unavailable".into()));
  assert_eq!(invites(cloud, invitee).await.invites.len(), 1, "the invite waits");
  cloud.publish_keys().await;
  let accepted = cloud.request("POST", &accept, Some(invitee), None).await;
  assert_eq!(accepted.status, 201);
  let accepted: InviteAccepted = accepted.json();
  remember(accepted.identity_token.expose());
  assert_eq!(
    (accepted.server.id, accepted.server.address.as_str()),
    (team.id, team.address.as_str())
  );
  the_token_names_the_invite(cloud, &accepted, team, invite).await;
  assert!(invites(cloud, invitee).await.invites.is_empty());
  assert_eq!(post(cloud, invitee, &accept).await, 404);
}

async fn only_the_invitee_declines(cloud: &TestCloud, owner: &str, invitee: &str, id: InviteId) {
  let decline = format!("/invites/{id}/decline");
  assert_eq!(post(cloud, owner, &decline).await, 404, "only the invitee declines");
  assert_eq!(post(cloud, invitee, &decline).await, 204);
  assert_eq!(post(cloud, invitee, &decline).await, 404);
  assert_eq!(post(cloud, invitee, "/invites/not-an-id/decline").await, 404);
}

async fn post(cloud: &TestCloud, token: &str, path: &str) -> u16 {
  cloud.request("POST", path, Some(token), None).await.status
}

/// The accepted invite's token verifies with the published key set for that server, and names the invite.
async fn the_token_names_the_invite(
  cloud: &TestCloud,
  accepted: &InviteAccepted,
  team: &TeamServer,
  invite: &InviteStatement,
) {
  let compact = cloud
    .request("GET", "/keys", None, None)
    .await
    .json::<KeySetResponse>()
    .key_set;
  let now = cloud.clock.now_ms() / 1000;
  let set = VerifiedKeySet::verify(&compact, &cloud.root.public_key(), now).expect("the key set");
  let claims = set
    .verify_token(accepted.identity_token.expose(), &team.id.to_string(), now)
    .expect("a token for the server")
    .claims;
  assert_eq!(claims.invite, Some(invite.id.to_string()));
  assert_eq!(claims.email, invite.email);
}

/// 10 invites a day per invitee email; an expired invite is no longer listed and the sweep deletes it.
pub async fn invites_are_limited_and_expire() {
  let cloud = TestCloud::start().await;
  let token = owner(&cloud, "owner@example.test").await;
  let team = TeamServer::generate();
  assert_eq!(cloud.link_server(&token, &team).await.status, 201);
  let email = "flooded@example.test";
  for _ in 0..10 {
    let invite = team.project_invite(email, cloud.clock.now_ms());
    assert_eq!(post_invite(&cloud, &team.sign_invite(&invite)).await.status, 202);
  }
  let eleventh = team.project_invite(email, cloud.clock.now_ms());
  let limited = post_invite(&cloud, &team.sign_invite(&eleventh)).await;
  assert_eq!((limited.status, limited.error()), (429, "rate_limited".into()));
  for index in 1..=10 {
    cloud.mail.nth(email, index).await;
  }
  cloud.register(email).await;
  let flooded = cloud.sign_in(email, PASSWORD, "Laptop").await;
  assert_eq!(invites(&cloud, flooded.token.expose()).await.invites.len(), 10);
  cloud.clock.advance(7 * DAY + 1000);
  let token = cloud.sign_in(email, PASSWORD, "Laptop").await;
  assert!(invites(&cloud, token.token.expose()).await.invites.is_empty());
  cloud.cloud.sweep().await.expect("sweep");
  let admin = postgres().database(&cloud.database).await;
  let left: i64 = admin
    .query_one("SELECT count(*) FROM invites", &[])
    .await
    .expect("count")
    .get(0);
  assert_eq!(left, 0);
  drop(admin);
  cloud.finish().await;
}
