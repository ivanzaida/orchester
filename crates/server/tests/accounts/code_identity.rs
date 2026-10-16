//! The cloud identity a code entered in an app shows (`docs/architecture/server.md`, "Codes"): a token issued for codes
//! only, which no join or sign-in with the identity takes, so a server cannot pass on as a join a token an app got for
//! a code there; one bound to a certificate, taken only by the server presenting it; and the identity of the code's
//! account, so a stolen account or pairing code shown with another person's two-factor identity gives nothing.

use orchester_api::{Credential, ErrorCode, InviteCreated, Joined, PairingCode};
use orchester_server::config::TlsMode;
use serde_json::{Value, json};

use crate::{
  cloud::{bob, cloud_join, linked, refused},
  support::TestServer,
};

#[tokio::test]
async fn code_identity_a_token_for_codes_neither_joins_nor_signs_in() {
  let (server, _admin, cloud, audience) = linked().await;
  let for_codes = cloud.code_token(&bob(&audience, None), (true, None), server.seconds());
  refused(
    &cloud_join(&server, &for_codes, None).await,
    401,
    ErrorCode::InvalidIdentity,
  );
  let browser = json!({"identity_token": for_codes, "device": "Edge on Windows", "kind": "admin_web"});
  refused(
    &server.request("POST", "/auth/cloud", None, Some(browser)).await,
    401,
    ErrorCode::InvalidIdentity,
  );
  server.finish().await;
}

/// A server that presents its own certificate, with an invite code from its first admin.
async fn self_signed_with_code() -> (TestServer, Credential) {
  let server = TestServer::start(|config| config.tls = TlsMode::SelfSigned).await;
  let setup = server
    .server
    .host_setup_code()
    .await
    .expect("setup")
    .expect("no admin yet");
  let body = json!({"code": setup.expose(), "device": "laptop", "username": "alice", "display_name": "Alice"});
  let admin: Joined = server
    .tls_pinned_request("POST", "/auth/code", None, Some(body))
    .await
    .json();
  server.two_factor(&admin.token);
  let invite: InviteCreated = server
    .tls_pinned_request(
      "POST",
      "/invites",
      Some(&admin.token),
      Some(json!({"kind":"server","delivery":"code"})),
    )
    .await
    .json();
  (server, invite.code.expect("a code"))
}

#[tokio::test]
async fn code_identity_a_bound_token_is_taken_only_where_its_certificate_is_presented() {
  let (server, code) = self_signed_with_code().await;
  let audience = server.audience().await;
  let cloud = server.cloud();
  let own = server.server.certificate().expect("its own certificate").sha256;
  let join = |token: String| {
    json!({"code": code.expose(), "device": "Bob's laptop", "username": "bob", "display_name": "Bob",
      "identity_token": token, "key_set": cloud.key_set(server.seconds())})
  };
  // What the app got for another server's certificate, passed on here.
  let elsewhere = cloud.code_token(&bob(&audience, None), (true, Some(&"ab".repeat(32))), server.seconds());
  refused(
    &server
      .tls_pinned_request("POST", "/auth/code", None, Some(join(elsewhere)))
      .await,
    401,
    ErrorCode::InvalidIdentity,
  );
  let bound = cloud.code_token(&bob(&audience, None), (true, Some(&own)), server.seconds());
  let joined = server
    .tls_pinned_request("POST", "/auth/code", None, Some(join(bound)))
    .await;
  assert_eq!(joined.status, 201, "the code stayed open for its own certificate");
  server.finish().await;
}

#[tokio::test]
async fn code_identity_a_stolen_account_code_takes_only_its_account_identity() {
  let server = TestServer::start(|_| {}).await;
  let admin = server.bootstrap().await;
  let bob = server.invited("bob", false).await;
  let account = server
    .server
    .host_account_code("bob".into())
    .await
    .expect("account code");
  refused(
    &server.redeem_as(&account, "mallory", "Mallory's laptop").await,
    401,
    ErrorCode::InvalidIdentity,
  );
  let again = server.join_as(&account, "bob", "new laptop").await;
  assert_eq!(
    again.user.id, bob.user.id,
    "the code stayed open for Bob's own identity"
  );

  // Alice made the server with its setup code, which shows no identity: the first her codes show is hers from then on,
  // unless it is another account's here.
  let pairing: PairingCode = server
    .request("POST", "/auth/pairings", Some(&admin.token), None)
    .await
    .json();
  refused(
    &server.redeem_as(&pairing.code, "bob", "phone").await,
    401,
    ErrorCode::InvalidIdentity,
  );
  server.join_as(&pairing.code, "alice", "phone").await;
  let pairing: PairingCode = server
    .request("POST", "/auth/pairings", Some(&admin.token), None)
    .await
    .json();
  refused(
    &server.redeem_as(&pairing.code, "mallory", "tablet").await,
    401,
    ErrorCode::InvalidIdentity,
  );
  server.finish().await;
}

#[tokio::test]
async fn code_identity_an_invite_code_makes_no_second_account_for_an_identity() {
  let server = TestServer::start(|_| {}).await;
  let admin = server.bootstrap().await;
  server.invited("bob", false).await;
  let invite: Value = server
    .request(
      "POST",
      "/invites",
      Some(&admin.token),
      Some(json!({"kind":"server","delivery":"code"})),
    )
    .await
    .json();
  let code = Credential::new(invite["code"].as_str().expect("invite code").into());
  let bobs = server.two_factor_proof("bob").await;
  refused(
    &server.redeem_with(&code, Some("robert"), "laptop", Some(bobs)).await,
    409,
    ErrorCode::Conflict,
  );
  assert_eq!(server.redeem(&code, Some("dana"), "laptop").await.status, 201);
  server.finish().await;
}
