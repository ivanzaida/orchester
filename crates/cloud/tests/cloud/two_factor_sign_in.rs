//! Signing in with two-factor authentication, and what a two-factor session may do (`docs/cloud-api.md`, "Two-factor
//! authentication"): the password step answers a challenge that one code finishes, once, within five minutes and five
//! tries, apart from the password limits; identity tokens say whether their device signed in with it; and only such a
//! session accepts invites, which list whether they need it.

use orchester_api::{
  cloud::{CloudInvites, IdentityTokenIssued, KeySetResponse, TokenPurpose, TwoFactorSignedIn},
  identity::{IdentityClaims, VerifiedKeySet},
};
use orchester_cloud::clock::Clock;
use orchester_core::id::InviteId;
use serde_json::{Value, json};

use crate::{
  registry::{TeamServer, link, signed_in},
  support::{MINUTE, PASSWORD, TestCloud, postgres, remember, two_factor::Authenticator},
  two_factor::status,
};

/// What `docs/cloud-hosting.md` gives the operator of a cloud to turn two-factor authentication off for an account
/// whose person lost their phone and recovery codes; the test runs it as written there.
const OPERATOR_RESET: &str = "WITH removed AS (
  DELETE FROM two_factor WHERE account = (SELECT id FROM accounts WHERE email = lower('person@example.com'))
  RETURNING account
)
INSERT INTO audit_events (action, target, at) SELECT 'two_factor_removed_by_operator', account::text, now() FROM removed;";

pub async fn the_password_step_answers_a_challenge_that_one_code_finishes() {
  let cloud = TestCloud::start().await;
  let email = "challenge@example.test";
  cloud.register(email).await;
  let laptop = cloud.sign_in(email, PASSWORD, "Laptop").await;
  let authenticator = cloud.enable_two_factor(laptop.token.expose()).await;
  let wrong = cloud.try_sign_in(email, "a wrong but long password", "Desktop").await;
  assert_eq!(
    (wrong.status, wrong.error()),
    (401, "invalid_credentials".into()),
    "no challenge without the password"
  );
  let challenge = cloud.challenge(email, "Desktop").await;
  assert!(challenge.starts_with("orcm_") && challenge.len() == 69);
  let refused = cloud.finish_sign_in(&challenge, "000000").await;
  assert_eq!((refused.status, refused.error()), (401, "invalid_code".into()));
  cloud.clock.advance(30 * 1000);
  let finished = cloud.finish_sign_in(&challenge, &authenticator.code(&cloud, 0)).await;
  assert_eq!(finished.status, 201);
  let signed: TwoFactorSignedIn = finished.json();
  assert_eq!(signed.recovery_codes_left, 10);
  assert_eq!(
    (
      signed.signed_in.account.email.as_str(),
      signed.signed_in.device.name.as_str()
    ),
    (email, "Desktop")
  );
  let desktop = signed.signed_in.token.expose();
  assert!(status(&cloud, desktop).await.this_session, "a two-factor session");
  challenges_end(&cloud, email, (&challenge, &authenticator)).await;
  cloud.finish().await;
}

/// A used, unknown or late challenge is `challenge_expired`; a recovery code finishes a fresh one; a browser's sign-in
/// is challenged too.
async fn challenges_end(cloud: &TestCloud, email: &str, (challenge, authenticator): (&str, &Authenticator)) {
  let reused = cloud.finish_sign_in(challenge, &authenticator.recovery_codes[0]).await;
  assert_eq!(
    (reused.status, reused.error()),
    (401, "challenge_expired".into()),
    "a challenge works once"
  );
  let unknown = cloud
    .finish_sign_in(&format!("orcm_{}", "0".repeat(64)), "000000")
    .await;
  assert_eq!((unknown.status, unknown.error()), (401, "challenge_expired".into()));
  let late = cloud.challenge(email, "Tablet").await;
  cloud.clock.advance(5 * MINUTE + 1000);
  let expired = cloud.finish_sign_in(&late, &authenticator.recovery_codes[0]).await;
  assert_eq!(
    (expired.status, expired.error()),
    (401, "challenge_expired".into()),
    "five minutes"
  );
  let by_recovery = cloud
    .finish_sign_in(&cloud.challenge(email, "Phone").await, &authenticator.recovery_codes[0])
    .await;
  assert_eq!(by_recovery.status, 201, "a recovery code finishes a sign-in");
  assert_eq!(by_recovery.json::<TwoFactorSignedIn>().recovery_codes_left, 9);
  let web = json!({"email": email, "password": PASSWORD, "device": {"name": "Firefox", "platform": "web"}});
  let browser = cloud.request("POST", "/auth/sessions", None, Some(web)).await;
  assert_eq!(browser.status, 202, "a browser's sign-in is challenged too");
  remember(
    browser.json::<serde_json::Value>()["challenge"]
      .as_str()
      .expect("challenge"),
  );
}

pub async fn code_tries_are_limited_per_challenge_and_per_account() {
  let cloud = TestCloud::start().await;
  let email = "tries@example.test";
  cloud.register(email).await;
  let laptop = cloud.sign_in(email, PASSWORD, "Laptop").await;
  let authenticator = cloud.enable_two_factor(laptop.token.expose()).await;
  cloud.clock.advance(30 * 1000);
  let challenge = cloud.challenge(email, "Desktop").await;
  for attempt in 0..5 {
    let wrong = cloud.finish_sign_in(&challenge, "000000").await;
    assert_eq!(wrong.error(), "invalid_code", "attempt {attempt}");
  }
  let spent = cloud.finish_sign_in(&challenge, &authenticator.code(&cloud, 0)).await;
  assert_eq!(
    (spent.status, spent.error()),
    (401, "challenge_expired".into()),
    "five tries a challenge"
  );
  let fresh = cloud.challenge(email, "Desktop").await;
  let limited = cloud.finish_sign_in(&fresh, &authenticator.code(&cloud, 0)).await;
  assert_eq!(
    (limited.status, limited.error()),
    (429, "rate_limited".into()),
    "five wrong codes an account"
  );
  let step_up = cloud
    .step_up(laptop.token.expose(), &authenticator.code(&cloud, 0))
    .await;
  assert_eq!(step_up.status, 429, "every code check of the account counts");
  assert_eq!(
    cloud.try_sign_in(email, PASSWORD, "Desktop").await.status,
    202,
    "the password limits are apart"
  );
  cloud.clock.advance(15 * MINUTE + 1000);
  let after = cloud
    .finish_sign_in(&cloud.challenge(email, "Desktop").await, &authenticator.code(&cloud, 0))
    .await;
  assert_eq!(after.status, 201);
  cloud.finish().await;
}

pub async fn a_password_change_ends_a_challenge() {
  let cloud = TestCloud::start().await;
  let email = "change@example.test";
  cloud.register(email).await;
  let laptop = cloud.sign_in(email, PASSWORD, "Laptop").await;
  let authenticator = cloud.enable_two_factor(laptop.token.expose()).await;
  let challenge = cloud.challenge(email, "Desktop").await;
  let changed_to = "another long passphrase";
  remember(changed_to);
  let body = json!({"current_password": PASSWORD, "new_password": changed_to});
  let changed = cloud
    .request("POST", "/auth/password", Some(laptop.token.expose()), Some(body))
    .await;
  assert_eq!(changed.status, 204);
  let finished = cloud.finish_sign_in(&challenge, &authenticator.recovery_codes[0]).await;
  assert_eq!((finished.status, finished.error()), (401, "challenge_expired".into()));
  cloud.finish().await;
}

/// The claims of a token for `server`, accepting `invite`, verified as the team server verifies them.
async fn claims(cloud: &TestCloud, device: &str, server: &TeamServer, invite: Option<InviteId>) -> IdentityClaims {
  claims_of(
    cloud,
    device,
    server,
    json!({"audience": server.id(), "invite": invite}),
  )
  .await
}

/// The claims of the token `body` asks for, verified as the team server verifies them.
async fn claims_of(cloud: &TestCloud, device: &str, server: &TeamServer, body: Value) -> IdentityClaims {
  let issued = cloud
    .request("POST", "/identity-tokens", Some(device), Some(body))
    .await;
  assert_eq!(issued.status, 201, "{}", issued.error());
  let issued: IdentityTokenIssued = issued.json();
  remember(issued.identity_token.expose());
  let keys: KeySetResponse = cloud.request("GET", "/keys", None, None).await.json();
  let now = cloud.clock.now_ms() / 1000;
  let set = VerifiedKeySet::verify(&keys.key_set, &cloud.root().await, now).expect("keys");
  set
    .verify_token(issued.identity_token.expose(), &server.id().to_string(), now)
    .expect("verifies")
    .claims
}

pub async fn invites_need_a_two_factor_session_and_tokens_say_so() {
  let cloud = TestCloud::start().await;
  let owner = signed_in(&cloud, "owner@example.test").await;
  let dana = signed_in(&cloud, "dana@example.test").await;
  let server = TeamServer::generate();
  link(&cloud, &owner, &server).await;
  let id = InviteId::generate();
  let body = json!({"invite": server.invite(id, "dana@example.test", cloud.clock.now_ms())});
  assert_eq!(cloud.request("POST", "/invites", None, Some(body)).await.status, 202);
  let accept = json!({"audience": server.id(), "invite": id});
  let needs = |listed: CloudInvites| {
    listed
      .invites
      .iter()
      .map(|invite| invite.needs_two_factor)
      .collect::<Vec<_>>()
  };
  let listed: CloudInvites = cloud.request("GET", "/invites", Some(&dana), None).await.json();
  assert_eq!(
    needs(listed),
    [true],
    "listed, saying it needs two-factor authentication"
  );
  let off = cloud
    .request("POST", "/identity-tokens", Some(&dana), Some(accept.clone()))
    .await;
  assert_eq!((off.status, off.error()), (403, "mfa_required".into()));
  assert!(
    !claims(&cloud, &dana, &server, None).await.mfa,
    "a join token says so too"
  );
  let earlier = cloud.sign_in("dana@example.test", PASSWORD, "Older laptop").await;
  cloud.enable_two_factor(&dana).await;
  let earlier = earlier.token.expose();
  let without = cloud
    .request("POST", "/identity-tokens", Some(earlier), Some(accept.clone()))
    .await;
  assert_eq!((without.status, without.error()), (403, "session_without_mfa".into()));
  let listed: CloudInvites = cloud.request("GET", "/invites", Some(earlier), None).await.json();
  assert_eq!(needs(listed), [true], "the invite stayed open");
  let listed: CloudInvites = cloud.request("GET", "/invites", Some(&dana), None).await.json();
  assert_eq!(needs(listed), [false]);
  let accepted = claims(&cloud, &dana, &server, Some(id)).await;
  assert!(
    accepted.mfa,
    "the token says its device signed in with two-factor authentication"
  );
  assert_eq!(accepted.invite, Some(id.to_string()));
  cloud.finish().await;
}

pub async fn an_operator_turns_it_off_with_the_documented_statement() {
  let documented = include_str!("../../../../docs/cloud-hosting.md");
  assert!(
    documented.contains(OPERATOR_RESET),
    "the statement is the one the operator reads"
  );
  let cloud = TestCloud::start().await;
  let email = "person@example.test";
  cloud.register(email).await;
  let laptop = cloud.sign_in(email, PASSWORD, "Laptop").await;
  let authenticator = cloud.enable_two_factor(laptop.token.expose()).await;
  let waiting = cloud.challenge(email, "Desktop").await;
  let admin = postgres().database(&cloud.database).await;
  let statement = OPERATOR_RESET.replace("person@example.com", email);
  let changed = admin
    .execute(statement.as_str(), &[])
    .await
    .expect("the operator's statement");
  assert_eq!(changed, 1, "it was on and is off now");
  assert_eq!(
    admin.execute(statement.as_str(), &[]).await.expect("again"),
    0,
    "off already"
  );
  let off = status(&cloud, laptop.token.expose()).await;
  assert!(!off.enabled && !off.this_session && off.recovery_codes_left == 0);
  let ended = cloud.finish_sign_in(&waiting, &authenticator.recovery_codes[0]).await;
  assert_eq!(ended.error(), "challenge_expired", "its open sign-ins end with it");
  assert_eq!(
    cloud.try_sign_in(email, PASSWORD, "Desktop").await.status,
    201,
    "the password signs in"
  );
  let audited: i64 = admin
    .query_one(
      "SELECT count(*) FROM audit_events WHERE action = 'two_factor_removed_by_operator'",
      &[],
    )
    .await
    .expect("audit")
    .get(0);
  assert_eq!(audited, 1);
  drop(admin);
  cloud.finish().await;
}

pub async fn tokens_for_codes_name_their_use_and_the_certificate_asked_for() {
  let cloud = TestCloud::start().await;
  let person = signed_in(&cloud, "codes@example.test").await;
  // Not linked here: what an app asks for a server the cloud does not know, bound to the certificate it pinned.
  let server = TeamServer::generate();
  let certificate = "ab".repeat(32);
  let body = json!({"audience": server.id(), "purpose": "code", "certificate": certificate});
  let bound = claims_of(&cloud, &person, &server, body).await;
  assert_eq!(bound.purpose, Some(TokenPurpose::Code));
  assert_eq!(bound.certificate.as_deref(), Some(certificate.as_str()));
  assert!(!bound.mfa, "a session without two-factor authentication says so");
  let unbound = claims_of(
    &cloud,
    &person,
    &server,
    json!({"audience": server.id(), "purpose": "code"}),
  )
  .await;
  assert_eq!(unbound.certificate, None);
  for asked in [
    json!({"audience": server.id(), "certificate": certificate}),
    json!({"audience": server.id(), "purpose": "admin", "certificate": certificate}),
    json!({"audience": server.id(), "purpose": "code", "certificate": "AB".repeat(32)}),
    json!({"audience": server.id(), "purpose": "code", "certificate": "ab"}),
    json!({"audience": server.id(), "purpose": "code", "invite": InviteId::generate()}),
  ] {
    let refused = cloud
      .request("POST", "/identity-tokens", Some(&person), Some(asked.clone()))
      .await;
    assert_eq!((refused.status, refused.error()), (422, "invalid".into()), "{asked}");
  }
  cloud.finish().await;
}
