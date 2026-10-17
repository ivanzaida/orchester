//! The cloud identity a code entered in an app shows (`docs/architecture/server.md`, "Codes"): a token issued for codes
//! only, which no join or sign-in with the identity takes; the server's proof that it holds the key of its id, which
//! the app asks for before it asks for that token; and the code's account. Redeeming a code links no identity, but for
//! an account code, which replaces the account's link and is audited: a leaked pairing code gives a device and nothing
//! of the account's identity; an account code brings back a person whose cloud account was made anew; an admin or the
//! host unlinks an account.

use orchester_api::{
  ErrorCode, Joined, KeyProofResponse, PairingCode, UserResponse,
  identity::{key_proof_nonce, read_statement, verify_key_proof},
  statements::{KEY_PROOF_TYPE, KeyProof},
};
use orchester_core::id::UserId;
use orchester_server::config::TlsMode;
use serde_json::json;

use crate::{
  cloud::{bob, cloud_join, linked, refused},
  support::{
    TestServer,
    cloud::{Identity, TestCloud, person},
  },
};

#[tokio::test]
async fn code_identity_a_token_for_codes_neither_joins_nor_signs_in() {
  let (server, _admin, cloud, audience) = linked().await;
  let for_codes = cloud.code_token(&bob(&audience, None), true, server.seconds());
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

/// The key proof the server answers `nonce` with, over its own TLS when it has it.
async fn key_proof(server: &TestServer, nonce: &str, tls: bool) -> KeyProofResponse {
  let body = Some(json!({"nonce": nonce}));
  let answer = if tls {
    server.tls_pinned_request("POST", "/server/key-proof", None, body).await
  } else {
    server.request("POST", "/server/key-proof", None, body).await
  };
  answer.json()
}

/// The certificate a key proof names, read without checking it.
fn named_certificate(answer: &KeyProofResponse) -> Option<String> {
  read_statement::<KeyProof>(&answer.proof, KEY_PROOF_TYPE)
    .expect("a key proof")
    .certificate
}

#[tokio::test]
async fn code_identity_the_server_proves_its_key_for_the_connection_s_certificate() {
  let server = TestServer::start(|config| config.tls = TlsMode::SelfSigned).await;
  let id = server.audience().await.parse().expect("the server's id");
  let own = server.server.certificate().expect("its own certificate").sha256;
  let nonce = key_proof_nonce().expect("a nonce");
  let answer = key_proof(&server, &nonce, true).await;
  verify_key_proof(&answer, id, (&nonce, &own)).expect("its key, for this nonce and its certificate");
  let other_nonce = key_proof_nonce().expect("a nonce");
  assert!(
    verify_key_proof(&answer, id, (&other_nonce, &own)).is_err(),
    "only for its nonce"
  );
  // Passed on by a server that presents another certificate.
  let elsewhere = "ab".repeat(32);
  assert!(
    verify_key_proof(&answer, id, (&nonce, &elsewhere)).is_err(),
    "only for its certificate"
  );
  let other = TestServer::start(|_| {}).await;
  let other_id = other.audience().await.parse().expect("another server's id");
  assert!(
    verify_key_proof(&answer, other_id, (&nonce, &own)).is_err(),
    "not another server's"
  );
  for nonce in ["short", "not a nonce at all, with spaces"] {
    let refused = server
      .tls_pinned_request("POST", "/server/key-proof", None, Some(json!({"nonce": nonce})))
      .await;
    assert_eq!(refused.status, 422, "{nonce}");
  }
  // Plain HTTP names no certificate, so no app takes its proof; one configured with its proxy's names that one.
  let plain = key_proof(&other, &nonce, false).await;
  assert_eq!(named_certificate(&plain), None);
  let proxied = TestServer::start(|config| config.proxy_certificate = Some("AB:".repeat(31) + "AB")).await;
  assert_eq!(
    named_certificate(&key_proof(&proxied, &nonce, false).await),
    Some("ab".repeat(32))
  );
  proxied.finish().await;
  other.finish().await;
  server.finish().await;
}

/// `person`'s cloud identity for the linked server `audience`.
fn identity<'a>(audience: &'a str, subject: &'a str) -> Identity<'a> {
  Identity {
    audience,
    subject,
    email: "person@example.com",
    name: "Person",
    invite: None,
  }
}

/// Links the account of the device `token` to `subject`'s cloud identity from that device (`POST /auth/cloud`).
async fn link(
  server: &TestServer,
  cloud: &TestCloud,
  (audience, token): (&str, &orchester_api::Credential),
  subject: &str,
) {
  let body = json!({"identity_token": cloud.token(&identity(audience, subject), server.seconds()), "device": "laptop"});
  let linked = server.request("POST", "/auth/cloud", Some(token), Some(body)).await;
  assert_eq!(linked.status, 200);
}

/// The cloud account `user` is linked to, if any.
fn subject_of(server: &TestServer, user: UserId) -> Option<String> {
  server
    .database()
    .query_row(
      "SELECT cloud_subject FROM users WHERE id=?1",
      [user.to_string()],
      |row| row.get(0),
    )
    .expect("the account")
}

async fn pairing(server: &TestServer, token: &orchester_api::Credential) -> PairingCode {
  server.request("POST", "/auth/pairings", Some(token), None).await.json()
}

#[tokio::test]
async fn code_identity_a_leaked_pairing_code_links_nothing_and_locks_nobody_out() {
  let (server, admin, cloud, audience) = linked().await;
  // Alice made the server with its setup code and has no cloud identity; Mallory redeems her leaked pairing code.
  let leaked = pairing(&server, &admin.token).await;
  server.join_as(&leaked.code, "mallory", "Mallory's laptop").await;
  assert_eq!(subject_of(&server, admin.user.id), None, "nothing linked");
  let mallorys = person("mallory");
  refused(
    &cloud_join(
      &server,
      &cloud.token(&identity(&audience, &mallorys), server.seconds()),
      None,
    )
    .await,
    403,
    ErrorCode::NotInvited,
  );
  // Alice links her own identity, and her pairing codes then take only it.
  let alices = person("alice");
  link(&server, &cloud, (&audience, &admin.token), &alices).await;
  let next = pairing(&server, &admin.token).await;
  refused(
    &server.redeem_as(&next.code, "mallory", "Mallory's phone").await,
    401,
    ErrorCode::InvalidIdentity,
  );
  let phone = server.join_as(&next.code, "alice", "Alice's phone").await;
  assert_eq!(phone.user.id, admin.user.id, "the code stayed open for Alice");
  server.finish().await;
}

#[tokio::test]
async fn code_identity_an_account_code_replaces_the_link_and_brings_back_a_new_cloud_account() {
  let (server, _admin, cloud, audience) = linked().await;
  let bob = server.invited("bob", false).await;
  let before = "0191c7c0-0000-7000-8000-0000000b0b01";
  link(&server, &cloud, (&audience, &bob.token), before).await;
  // Bob's cloud account was deleted and made again: a new identity. The host gives him an account code.
  let account = server
    .server
    .host_account_code("bob".into())
    .await
    .expect("account code");
  let again = server.join_as(&account, "bob", "new laptop").await;
  assert_eq!(again.user.id, bob.user.id);
  assert_eq!(subject_of(&server, bob.user.id), Some(person("bob")), "replaced");
  let relinked: i64 = server
    .database()
    .query_row(
      "SELECT COUNT(*) FROM audit_events WHERE action='cloud_relinked'",
      [],
      |row| row.get(0),
    )
    .expect("audit");
  assert_eq!(relinked, 1);
  let old = cloud_join(
    &server,
    &cloud.token(&identity(&audience, before), server.seconds()),
    None,
  )
  .await;
  refused(&old, 403, ErrorCode::NotInvited);
  let bobs = person("bob");
  let joined: Joined = cloud_join(
    &server,
    &cloud.token(&identity(&audience, &bobs), server.seconds()),
    None,
  )
  .await
  .json();
  assert_eq!(joined.user.id, bob.user.id, "his new cloud account signs in as him");

  // An identity another account here is linked to is not taken.
  let carol = server.invited("carol", false).await;
  let carols = person("carol");
  link(&server, &cloud, (&audience, &carol.token), &carols).await;
  let account = server
    .server
    .host_account_code("bob".into())
    .await
    .expect("account code");
  refused(
    &server.redeem_as(&account, "carol", "laptop").await,
    401,
    ErrorCode::InvalidIdentity,
  );
  server.finish().await;
}

#[tokio::test]
async fn code_identity_an_admin_or_the_host_unlinks_an_account() {
  let (server, admin, cloud, audience) = linked().await;
  server.two_factor(&admin.token);
  let bob = server.invited("bob", false).await;
  let bobs = person("bob");
  link(&server, &cloud, (&audience, &bob.token), &bobs).await;
  let path = format!("/admin/users/{}/cloud", bob.user.id);
  refused(
    &server.request("DELETE", &path, Some(&bob.token), None).await,
    403,
    ErrorCode::Forbidden,
  );
  let unlinked: UserResponse = server.request("DELETE", &path, Some(&admin.token), None).await.json();
  assert!(!unlinked.user.cloud);
  assert_eq!(subject_of(&server, bob.user.id), None);
  refused(
    &cloud_join(
      &server,
      &cloud.token(&identity(&audience, &bobs), server.seconds()),
      None,
    )
    .await,
    403,
    ErrorCode::NotInvited,
  );
  let carol = server.invited("carol", false).await;
  let carols = person("carol");
  link(&server, &cloud, (&audience, &carol.token), &carols).await;
  let shown = server
    .server
    .host_account_cloud("carol".into(), false)
    .await
    .expect("shown");
  assert_eq!(shown, Some(carols));
  let left = server
    .server
    .host_account_cloud("carol".into(), true)
    .await
    .expect("unlinked");
  assert_eq!(left, None);
  let audited: i64 = server
    .database()
    .query_row(
      "SELECT COUNT(*) FROM audit_events WHERE action='cloud_unlinked'",
      [],
      |row| row.get(0),
    )
    .expect("audit");
  assert_eq!(audited, 2);
  server.finish().await;
}

#[tokio::test]
async fn code_identity_an_invite_code_makes_no_second_account_for_an_identity() {
  let (server, admin, cloud, audience) = linked().await;
  server.two_factor(&admin.token);
  let bob = server.invited("bob", false).await;
  assert!(!bob.user.cloud, "an invite code links nothing");
  let bobs = person("bob");
  link(&server, &cloud, (&audience, &bob.token), &bobs).await;
  let invite: serde_json::Value = server
    .request(
      "POST",
      "/invites",
      Some(&admin.token),
      Some(json!({"kind":"server","delivery":"code"})),
    )
    .await
    .json();
  let code = orchester_api::Credential::new(invite["code"].as_str().expect("invite code").into());
  let proof = server.two_factor_proof("bob").await;
  refused(
    &server.redeem_with(&code, Some("robert"), "laptop", Some(proof)).await,
    409,
    ErrorCode::AccountExists,
  );
  let dana = server.join(&code, Some("dana"), "laptop").await;
  assert!(!dana.user.cloud, "the code stayed open, and links nothing");
  server.finish().await;
}
