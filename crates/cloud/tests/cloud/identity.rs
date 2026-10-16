use orchester_api::{
  cloud::{CloudPlatform, IdentityTokenIssued, KeySetResponse},
  identity::{CLOCK_SKEW_SECS, IdentityError, TOKEN_LIFETIME_SECS, VerifiedKeySet},
};
use serde_json::json;

use crate::support::{DAY, PASSWORD, TestCloud, postgres, remember};

const SERVER: &str = "0199f0a0-0000-7000-8000-00000000c10d";

fn seconds(cloud: &TestCloud) -> i64 {
  use orchester_cloud::clock::Clock;
  cloud.clock.now_ms() / 1000
}

async fn key_set(cloud: &TestCloud) -> VerifiedKeySet {
  let response = cloud.request("GET", "/keys", None, None).await;
  assert_eq!(response.status, 200);
  let compact = response.json::<KeySetResponse>().key_set;
  VerifiedKeySet::verify(&compact, &cloud.root().await, seconds(cloud)).expect("the key set verifies with the root")
}

async fn identity_token(cloud: &TestCloud, device: &str) -> String {
  let response = cloud
    .request(
      "POST",
      "/identity-tokens",
      Some(device),
      Some(json!({"audience": SERVER})),
    )
    .await;
  assert_eq!(response.status, 201);
  let issued: IdentityTokenIssued = response.json();
  remember(issued.identity_token.expose());
  issued.identity_token.expose().to_owned()
}

pub async fn identity_token_verifies_with_the_published_key_set() {
  let cloud = TestCloud::start().await;
  let email = "tokens@example.test";
  cloud.register(email).await;
  let signed = cloud.sign_in(email, PASSWORD, "Workstation").await;
  let device = signed.token.expose();
  let set = key_set(&cloud).await;
  let token = identity_token(&cloud, device).await;
  let now = seconds(&cloud);
  let claims = set
    .verify_token(&token, SERVER, now)
    .expect("the token verifies")
    .claims;
  assert_eq!(claims.sub, signed.account.id);
  assert_eq!(claims.email, email);
  assert_eq!(claims.name, "Ada Lovelace");
  assert_eq!(
    (claims.device.name.as_str(), claims.device.platform),
    ("Workstation", CloudPlatform::Windows)
  );
  assert_eq!(claims.iss, "https://cloud.example.test");

  let other = "0199f0a0-0000-7000-8000-00000000beef";
  assert_eq!(set.verify_token(&token, other, now), Err(IdentityError::WrongAudience));
  let (signed_part, signature) = token.rsplit_once('.').expect("compact JWS");
  let flipped = if signature.starts_with('A') { 'B' } else { 'A' };
  let forged = format!("{signed_part}.{flipped}{}", &signature[1..]);
  assert_eq!(set.verify_token(&forged, SERVER, now), Err(IdentityError::BadSignature));
  let late = now + TOKEN_LIFETIME_SECS + CLOCK_SKEW_SECS;
  assert_eq!(set.verify_token(&token, SERVER, late), Err(IdentityError::Expired));

  let invalid = cloud
    .request(
      "POST",
      "/identity-tokens",
      Some(device),
      Some(json!({"audience": "https://team.example.test"})),
    )
    .await;
  assert_eq!(invalid.status, 422);
  assert_eq!(
    cloud
      .request("POST", "/identity-tokens", None, Some(json!({"audience": SERVER})))
      .await
      .status,
    401
  );
  cloud.finish().await;
}

pub async fn identity_key_sets_are_signed_weekly_and_renewed_after_a_stop() {
  let cloud = TestCloud::start().await;
  let email = "weekly@example.test";
  cloud.register(email).await;
  let signed = cloud.sign_in(email, PASSWORD, "Workstation").await;
  let device = signed.token.expose();
  let issued_at = key_set(&cloud).await.statement().issued_at;

  // Within the first key's first three weeks nothing rotates; the weekly key set lists it alone.
  cloud.clock.advance(8 * DAY);
  cloud.cloud.keep_keys().await.expect("keep the keys");
  let weekly = key_set(&cloud).await;
  assert!(
    weekly.statement().issued_at > issued_at,
    "a week on, a new key set is signed"
  );
  assert_eq!(weekly.statement().keys.len(), 1);

  // A key set the cloud did not renew (it was stopped for a month) expires, and the cloud stops issuing tokens
  // rather than sign unverifiable ones, until it keeps its keys again.
  cloud.clock.advance(31 * DAY);
  let stale = cloud
    .request(
      "POST",
      "/identity-tokens",
      Some(device),
      Some(json!({"audience": SERVER})),
    )
    .await;
  assert_eq!((stale.status, stale.error()), (503, "keys_unavailable".into()));
  cloud.cloud.keep_keys().await.expect("keep the keys");
  key_set(&cloud).await;
  identity_token(&cloud, device).await;
  cloud.finish().await;
}

pub async fn identity_key_rotation_keeps_the_previous_key_then_drops_it() {
  let cloud = TestCloud::start().await;
  let email = "rotation@example.test";
  cloud.register(email).await;
  let signed = cloud.sign_in(email, PASSWORD, "Workstation").await;
  let device = signed.token.expose();
  let first = identity_token(&cloud, device).await;
  let issued_at = seconds(&cloud);

  // A week before the first key stops signing, a rotation lists the new key beside it; the new key signs only two
  // days later, once team servers have it (review 1, L4), and the previous one signs until then.
  cloud.clock.advance(24 * DAY);
  cloud.cloud.keep_keys().await.expect("keep the keys");
  let both = key_set(&cloud).await;
  assert_eq!(both.statement().keys.len(), 2);
  assert!(
    both.verify_token(&first, SERVER, issued_at + 60).is_ok(),
    "the previous key still verifies"
  );
  let overlap = identity_token(&cloud, device).await;
  assert_eq!(kid(&first), kid(&overlap), "the new key does not sign yet");
  cloud.clock.advance(2 * DAY);
  let second = identity_token(&cloud, device).await;
  assert_ne!(kid(&first), kid(&second));

  // Once the first key's 90 days are over, a new key set no longer lists it.
  cloud.clock.advance(65 * DAY);
  cloud.cloud.keep_keys().await.expect("keep the keys");
  let later = key_set(&cloud).await;
  assert!(later.statement().keys.iter().all(|key| key.kid != kid(&first)));
  assert_eq!(
    later.verify_token(&first, SERVER, issued_at + 60),
    Err(IdentityError::UnknownKey)
  );
  identity_token(&cloud, device).await;
  cloud.finish().await;
}

fn kid(token: &str) -> String {
  use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
  let header = URL_SAFE_NO_PAD
    .decode(token.split('.').next().expect("header"))
    .expect("base64url");
  serde_json::from_slice::<serde_json::Value>(&header).expect("JSON")["kid"]
    .as_str()
    .expect("kid")
    .to_owned()
}

/// Review 1, M4: more concurrent issuances than the pool has connections (16) all succeed; an issuance holds one
/// connection, never two. The accounts table is locked while the requests come in, so they all hold their
/// connection at once when it is released.
pub async fn many_concurrent_identity_tokens_do_not_starve_the_pool() {
  let cloud = TestCloud::start().await;
  let email = "busy@example.test";
  cloud.register(email).await;
  let signed = cloud.sign_in(email, PASSWORD, "Workstation").await;
  let bearer = format!("Bearer {}", signed.token.expose());
  let body = serde_json::to_vec(&json!({"audience": SERVER})).expect("JSON");
  let mut admin = postgres().database(&cloud.database).await;
  let lock = admin.transaction().await.expect("a locking transaction");
  lock
    .batch_execute("LOCK TABLE accounts IN ACCESS EXCLUSIVE MODE")
    .await
    .expect("lock accounts");
  let mut running = tokio::task::JoinSet::new();
  for _ in 0..24 {
    let (address, bearer, body) = (cloud.address(), bearer.clone(), body.clone());
    running.spawn(async move {
      let headers = [("Content-Type", "application/json"), ("Authorization", bearer.as_str())];
      crate::support::http::exchange(address, "POST", "/api/v1/identity-tokens", &headers, &body).await
    });
  }
  tokio::time::sleep(std::time::Duration::from_secs(2)).await;
  lock.commit().await.expect("release accounts");
  while let Some(answer) = running.join_next().await {
    let answer = answer.expect("the request task");
    assert_eq!(answer.status, 201);
    remember(answer.json::<IdentityTokenIssued>().identity_token.expose());
  }
  drop(admin);
  cloud.finish().await;
}
