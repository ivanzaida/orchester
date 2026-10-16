//! Accepting an invite (N1): the invitee's app asks for a join token that names the invite, which only the invitee
//! gets, only from the invite's server and only while the invite is open. The invite is then accepted: it leaves the
//! list, and the same account can ask again (a join that failed is retried) until it expires; nobody else can, a
//! decline cannot follow, and delivering it again brings nothing back (N1 security review 1, M2 and minor 1).

use orchester_api::{
  cloud::{CloudInvites, IdentityTokenIssued, KeySetResponse, ServersResponse},
  identity::VerifiedKeySet,
};
use orchester_cloud::clock::Clock;
use orchester_core::id::InviteId;
use serde_json::json;

use crate::{
  registry::{TeamServer, link, signed_in},
  support::{TestCloud, remember},
};

pub async fn invites_are_accepted_with_a_token_that_names_them() {
  let cloud = TestCloud::start().await;
  cloud.publish_keys().await;
  let owner = signed_in(&cloud, "ivan@example.test").await;
  let dana = signed_in(&cloud, "dana@example.test").await;
  let (server, other) = (TeamServer::generate(), TeamServer::generate());
  link(&cloud, &owner, &server).await;
  link(&cloud, &owner, &other).await;
  let id = InviteId::generate();
  let body = json!({"invite": server.invite(id, "dana@example.test", cloud.clock.now_ms())});
  assert_eq!(
    cloud.request("POST", "/invites", None, Some(body.clone())).await.status,
    202
  );
  let accept = json!({"audience": server.id(), "invite": id});
  refused_unless_open_to_the_invitee(&cloud, (&owner, &dana), (&other, id)).await;
  token_names_the_invite(&cloud, &dana, (&server, id)).await;
  accepted_is_asked_again_until_it_expires(&cloud, &dana, (id, &body, &accept)).await;
  cloud.finish().await;
}

/// The token names the invite, is a join token, and the invite leaves the list while the join is recorded.
async fn token_names_the_invite(cloud: &TestCloud, dana: &str, (server, id): (&TeamServer, InviteId)) {
  let accept = json!({"audience": server.id(), "invite": id});
  let issued = cloud
    .request("POST", "/identity-tokens", Some(dana), Some(accept))
    .await;
  assert_eq!(issued.status, 201, "{}", issued.error());
  let issued: IdentityTokenIssued = issued.json();
  remember(issued.identity_token.expose());
  let keys: KeySetResponse = cloud.request("GET", "/keys", None, None).await.json();
  let now = cloud.clock.now_ms() / 1000;
  let set = VerifiedKeySet::verify(&keys.key_set, &cloud.root.public_key(), now).expect("keys");
  let claims = set
    .verify_token(issued.identity_token.expose(), &server.id().to_string(), now)
    .expect("verifies")
    .claims;
  assert_eq!(claims.invite, Some(id.to_string()), "the token names the invite");
  assert_eq!(claims.purpose, None, "a join token");
  let listed: CloudInvites = cloud.request("GET", "/invites", Some(dana), None).await.json();
  assert!(listed.invites.is_empty(), "an accepted invite leaves the list");
  let servers: ServersResponse = cloud.request("GET", "/servers", Some(dana), None).await.json();
  assert_eq!(servers.servers.len(), 1, "the join is recorded");
}

/// The same account asks again (a join that failed); a decline cannot follow, a second delivery brings nothing back,
/// and the invite's expiry ends it.
async fn accepted_is_asked_again_until_it_expires(
  cloud: &TestCloud,
  dana: &str,
  (id, body, accept): (InviteId, &serde_json::Value, &serde_json::Value),
) {
  let again = cloud
    .request("POST", "/identity-tokens", Some(dana), Some(accept.clone()))
    .await;
  assert_eq!(again.status, 201, "a join that failed is retried with a new token");
  remember(again.json::<IdentityTokenIssued>().identity_token.expose());
  let decline = format!("/invites/{id}/decline");
  assert_eq!(
    cloud.request("POST", &decline, Some(dana), None).await.status,
    404,
    "accepted, not open"
  );
  assert_eq!(
    cloud.request("POST", "/invites", None, Some(body.clone())).await.status,
    202
  );
  let listed: CloudInvites = cloud.request("GET", "/invites", Some(dana), None).await.json();
  assert!(listed.invites.is_empty(), "delivering it again brings nothing back");
  cloud.clock.advance(8 * crate::support::DAY);
  let expired = cloud
    .request("POST", "/identity-tokens", Some(dana), Some(accept.clone()))
    .await;
  assert_eq!(expired.status, 404, "until it expires");
}

/// Someone else, another server's audience, and an invite with a purpose are refused, and the invite stays open.
async fn refused_unless_open_to_the_invitee(
  cloud: &TestCloud,
  (owner, dana): (&str, &str),
  (other, id): (&TeamServer, InviteId),
) {
  let server = cloud
    .request("GET", "/invites", Some(dana), None)
    .await
    .json::<CloudInvites>()
    .invites[0]
    .server
    .id;
  let stranger = json!({"audience": server, "invite": id});
  let refused = cloud
    .request("POST", "/identity-tokens", Some(owner), Some(stranger))
    .await;
  assert_eq!(refused.status, 404, "only the invitee accepts");
  let elsewhere = json!({"audience": other.id(), "invite": id});
  let refused = cloud
    .request("POST", "/identity-tokens", Some(dana), Some(elsewhere))
    .await;
  assert_eq!(refused.status, 404, "only for the invite's server");
  let admin = json!({"audience": server, "invite": id, "purpose": "admin"});
  let refused = cloud.request("POST", "/identity-tokens", Some(dana), Some(admin)).await;
  assert_eq!((refused.status, refused.error()), (422, "invalid".into()));
  let listed: CloudInvites = cloud.request("GET", "/invites", Some(dana), None).await.json();
  assert_eq!(listed.invites.len(), 1, "still open");
}
