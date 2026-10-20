//! Two-factor authentication on the team server (`docs/architecture/server.md`, "Invites", "Codes"): an invite, by
//! email or with a code, is made only from a device token issued for a cloud identity that signed in with two-factor
//! authentication (the identity token's `mfa` claim), or from a browser signed in with a pairing code such a token
//! made. A join token that accepts an invite must carry the claim, and so must the identity token for codes an app
//! sends with every code but the setup code, verified against the pinned root (`code_identity.rs`: what else binds it).

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

/// A browser signed in to the admin with `code`.
async fn admin_with_code(server: &TestServer, code: &Credential) -> Joined {
  let body = json!({"code": code.expose(), "device": "Edge on Windows", "kind": "admin_web"});
  let response = server.request("POST", "/auth/code", None, Some(body)).await;
  assert_eq!(response.status, 201);
  response.json()
}

#[tokio::test]
async fn two_factor_invites_are_made_only_from_a_two_factor_sign_in() {
  let (server, _admin, cloud, audience) = linked_admin().await;
  let by_email = json!({"kind":"server","delivery":"cloud","email":"dana@example.com"});
  let by_code = json!({"kind":"server","delivery":"code"});
  let identity = bob(&audience, None);
  let without = admin_browser(
    &server,
    cloud.purposed_token(&identity, TokenPurpose::Admin, server.seconds()),
  )
  .await;
  let account_code = server
    .server
    .host_account_code("alice".into())
    .await
    .expect("account code");
  let with_code = admin_with_code(&server, &account_code).await;
  // The admin reads from its sign-in whether invites will go: the session says how it signed in.
  assert!(!without.session.mfa && !with_code.session.mfa);
  for token in [&without.token, &with_code.token] {
    for body in [&by_email, &by_code] {
      refused(&send(&server, token, body).await, 403, ErrorCode::MfaRequired);
    }
  }
  let malformed = send(&server, &without.token, &json!({"kind":"server","delivery":"cloud"})).await;
  assert_eq!(malformed.status, 422, "what an invite may be is checked first");
  // Turning two-factor authentication on in the cloud changes no session here: only signing in again with it does.
  let with = admin_browser(&server, cloud.two_factor_admin_token(&identity, server.seconds())).await;
  assert!(with.session.mfa);
  refused(
    &send(&server, &without.token, &by_email).await,
    403,
    ErrorCode::MfaRequired,
  );
  let sent = send(&server, &with.token, &by_email).await;
  assert_eq!(sent.status, 201);
  let sent: InviteCreated = sent.json();
  assert!(sent.signed.is_some(), "signed for the cloud");
  assert_eq!(send(&server, &with.token, &by_code).await.status, 201);
  let revoke = format!("/invites/{}", sent.invite.id);
  let revoked = server.request("DELETE", &revoke, Some(&without.token), None).await;
  assert_eq!(revoked.status, 200, "revoking is not sending");
  server.finish().await;
}

/// Joins with `code` as an app does, with `proof` (an identity token and its key set) or none.
async fn redeem(server: &TestServer, code: &Credential, proof: Option<(&str, &str)>) -> Response {
  let mut body = json!({"code": code.expose(), "device": "Bob's laptop", "username": "bob", "display_name": "Bob"});
  if let Some((token, key_set)) = proof {
    body["identity_token"] = json!(token);
    body["key_set"] = json!(key_set);
  }
  server.request("POST", "/auth/code", None, Some(body)).await
}

#[tokio::test]
async fn two_factor_a_code_entered_in_an_app_needs_a_two_factor_identity() {
  let server = TestServer::start(|_| {}).await;
  let admin = server.bootstrap().await;
  let invite: InviteCreated = send(
    &server,
    &admin.token,
    &json!({"kind":"server","delivery":"code","admin":true}),
  )
  .await
  .json();
  let code = invite.code.expect("a code");
  refused(&redeem(&server, &code, None).await, 403, ErrorCode::MfaUnverifiable);
  let (valid, key_set) = server.two_factor_proof("bob").await;
  let audience = server.audience().await;
  refused(&redeem(&server, &code, None).await, 403, ErrorCode::MfaRequired);
  let cloud = server.cloud();
  let now = server.seconds();
  let without = cloud.code_token(&bob(&audience, None), false, now);
  refused(
    &redeem(&server, &code, Some((&without, &key_set))).await,
    403,
    ErrorCode::MfaRequired,
  );
  // A token for another server, or for another use than codes, is none for this code.
  let elsewhere = cloud.code_token(&bob("0199f0a0-0000-7000-8000-00000000e15e", None), true, now);
  let to_join = cloud.token_mfa(&bob(&audience, None), true, now);
  let for_admin = cloud.two_factor_admin_token(&bob(&audience, None), now);
  for token in [elsewhere, to_join, for_admin] {
    refused(
      &redeem(&server, &code, Some((&token, &key_set))).await,
      401,
      ErrorCode::InvalidIdentity,
    );
  }
  server
    .server
    .host_cloud(orchester_server::host::CloudCommand::Off)
    .await
    .expect("cloud off");
  let joined = redeem(&server, &code, Some((&valid, &key_set))).await;
  assert_eq!(
    joined.status, 201,
    "the code stayed open, and cloud sign-ins being off does not matter"
  );
  let joined: Joined = joined.json();
  let made = send(&server, &joined.token, &json!({"kind":"server","delivery":"code"})).await;
  assert_eq!(made.status, 201, "a device that joined with two-factor makes invites");
  refused(
    &redeem(&server, &code, Some((&valid, &key_set))).await,
    401,
    ErrorCode::InvalidCode,
  );
  server.finish().await;
}

#[tokio::test]
async fn two_factor_a_pairing_code_carries_it_to_the_admin() {
  let server = TestServer::start(|_| {}).await;
  let admin = server.bootstrap().await;
  let computer = server.another_device(&admin.token).await;
  let pairing = server
    .request("POST", "/auth/pairings", Some(&computer.token), None)
    .await;
  let pairing: orchester_api::PairingCode = pairing.json();
  let browser = admin_with_code(&server, &pairing.code).await;
  let made = send(&server, &browser.token, &json!({"kind":"server","delivery":"code"})).await;
  assert_eq!(made.status, 201, "the pairing code came from a device with two-factor");
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
