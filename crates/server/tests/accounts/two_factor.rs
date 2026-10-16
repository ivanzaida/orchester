//! Two-factor authentication on the team server (`docs/architecture/server.md`, "Invites"): an invite by email is sent
//! only from a device token issued for a cloud identity that signed in with two-factor authentication (the identity
//! token's `mfa` claim), and a join token that accepts an invite must carry the claim. Codes need no cloud and are not
//! held to it.

use orchester_api::{Credential, ErrorCode, InviteCreated, Joined, cloud::TokenPurpose};
use serde_json::{Value, json};

use crate::{
  cloud::{bob, cloud_join, email_invite, linked, refused},
  support::{Response, TestServer, cloud::TestCloud},
};

/// A server linked to a test cloud whose first admin's account is Bob's cloud identity, as the link's identity token
/// makes it.
async fn linked_admin() -> (TestServer, Joined, TestCloud, String) {
  let (server, admin, cloud, audience) = linked().await;
  let start = server
    .request(
      "PATCH",
      "/server/cloud",
      Some(&admin.token),
      Some(json!({"link":"start"})),
    )
    .await;
  assert_eq!(start.status, 200);
  let token = cloud.purposed_token(&bob(&audience, None), TokenPurpose::Link, server.seconds());
  let finish = json!({"link": {"root_key": cloud.root.public(), "key_set": cloud.key_set(server.seconds()),
    "identity_token": token}});
  let finished = server
    .request("PATCH", "/server/cloud", Some(&admin.token), Some(finish))
    .await;
  assert_eq!(finished.status, 200);
  (server, admin, cloud, audience)
}

/// A browser signed in to the admin with an identity token issued for it.
async fn admin_browser(server: &TestServer, token: String) -> Joined {
  let body = json!({"identity_token": token, "device": "Edge on Windows", "kind": "admin_web"});
  let response = server.request("POST", "/auth/cloud", None, Some(body)).await;
  assert_eq!(response.status, 201);
  response.json()
}

async fn send(server: &TestServer, token: &Credential, body: &Value) -> Response {
  server
    .request("POST", "/invites", Some(token), Some(body.clone()))
    .await
}

#[tokio::test]
async fn two_factor_email_invites_are_sent_only_from_a_two_factor_sign_in() {
  let (server, admin, cloud, audience) = linked_admin().await;
  let by_email = json!({"kind":"server","delivery":"cloud","email":"dana@example.com"});
  // A computer joined with a code: no cloud sign-in vouches for it.
  let computer = server.another_device(&admin.token).await;
  refused(
    &send(&server, &computer.token, &by_email).await,
    403,
    ErrorCode::MfaRequired,
  );
  let code = send(&server, &computer.token, &json!({"kind":"server","delivery":"code"})).await;
  assert_eq!(code.status, 201, "a code needs no cloud");
  let identity = bob(&audience, None);
  let without = admin_browser(
    &server,
    cloud.purposed_token(&identity, TokenPurpose::Admin, server.seconds()),
  )
  .await;
  refused(
    &send(&server, &without.token, &by_email).await,
    403,
    ErrorCode::MfaRequired,
  );
  let malformed = send(&server, &without.token, &json!({"kind":"server","delivery":"cloud"})).await;
  assert_eq!(malformed.status, 422, "what an invite may be is checked first");
  let with = admin_browser(&server, cloud.two_factor_admin_token(&identity, server.seconds())).await;
  let sent = send(&server, &with.token, &by_email).await;
  assert_eq!(sent.status, 201);
  let sent: InviteCreated = sent.json();
  assert!(sent.signed.is_some(), "signed for the cloud");
  let revoke = format!("/invites/{}", sent.invite.id);
  let revoked = server.request("DELETE", &revoke, Some(&without.token), None).await;
  assert_eq!(revoked.status, 200, "revoking is not sending");
  server.finish().await;
}

#[tokio::test]
async fn two_factor_a_token_that_accepts_an_invite_needs_the_claim() {
  let (server, admin, cloud, audience) = linked().await;
  let invite = email_invite(
    &server,
    &admin.token,
    json!({"kind":"server","delivery":"cloud","email":"bob@example.com"}),
  )
  .await;
  let id = invite.invite.id.to_string();
  let without = cloud.token_mfa(&bob(&audience, Some(&id)), false, server.seconds());
  refused(&cloud_join(&server, &without, None).await, 403, ErrorCode::MfaRequired);
  let with = cloud.token_mfa(&bob(&audience, Some(&id)), true, server.seconds());
  let joined = cloud_join(&server, &with, None).await;
  assert_eq!(joined.status, 201, "the invite stayed open for a token with the claim");
  server.finish().await;
}
