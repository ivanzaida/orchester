//! Orchester Cloud's identity tokens and key sets over explicit keys and times (`identity` feature).

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use orchester_api::{
  cloud::{CloudPlatform, DeviceInfo},
  identity::{
    CLOCK_SKEW_SECS, IdentityClaims, IdentityError, KeySetStatement, ListedKey, PrivateKey, RootKey, SigningKey,
    TOKEN_LIFETIME_SECS, TokenSubject, VerifiedKeySet,
  },
};
use orchester_core::id::CloudAccountId;

const ISSUER: &str = "https://cloud.example.test";
const SERVER: &str = "0199f0a0-0000-7000-8000-000000000001";
const NOW: i64 = 1_791_000_000;
const DAY: i64 = 24 * 60 * 60;

fn root() -> RootKey {
  RootKey::new(&PrivateKey::generate().expect("root key")).expect("root")
}

fn signing_key() -> SigningKey {
  SigningKey::new(&PrivateKey::generate().expect("signing key")).expect("key")
}

fn listed(key: &SigningKey, not_before: i64) -> ListedKey {
  ListedKey {
    kid: key.kid().into(),
    public_key: key.public_key().encoded(),
    not_before,
    signs_until: not_before + 30 * DAY,
    listed_until: not_before + 90 * DAY,
  }
}

fn key_set(root: &RootKey, keys: Vec<ListedKey>, issued_at: i64) -> VerifiedKeySet {
  let compact = root
    .sign_key_set(&KeySetStatement::new(ISSUER.into(), keys, issued_at))
    .expect("sign key set");
  VerifiedKeySet::verify(&compact, &root.public_key(), issued_at).expect("verify key set")
}

fn token(key: &SigningKey, audience: &str, now: i64) -> String {
  let claims = IdentityClaims::new(
    TokenSubject {
      issuer: ISSUER.into(),
      audience: audience.into(),
      account: CloudAccountId::generate(),
      email: "ada@example.test".into(),
      name: "Ada".into(),
      device: DeviceInfo {
        name: "Ada's laptop".into(),
        platform: CloudPlatform::Macos,
      },
    },
    now,
  )
  .expect("claims");
  key.sign_token(&claims).expect("sign").expose().to_owned()
}

#[test]
fn identity_token_verifies_with_root_signed_key_set() {
  let root = root();
  let key = signing_key();
  let set = key_set(&root, vec![listed(&key, NOW - DAY)], NOW);
  let claims = set
    .verify_token(&token(&key, SERVER, NOW), SERVER, NOW + 10)
    .expect("valid token");
  assert_eq!(claims.aud, SERVER);
  assert_eq!(claims.iss, ISSUER);
  assert_eq!(claims.exp - claims.iat, TOKEN_LIFETIME_SECS);
  assert_eq!(claims.device.platform, CloudPlatform::Macos);
  let other = set
    .verify_token(&token(&key, SERVER, NOW), SERVER, NOW)
    .expect("second token");
  assert_ne!(claims.jti, other.jti, "every token has its own id");
}

#[test]
fn identity_token_expires_after_its_lifetime_and_the_clock_skew() {
  let root = root();
  let key = signing_key();
  let set = key_set(&root, vec![listed(&key, NOW - DAY)], NOW);
  let token = token(&key, SERVER, NOW);
  let last = NOW + TOKEN_LIFETIME_SECS + CLOCK_SKEW_SECS - 1;
  assert!(set.verify_token(&token, SERVER, last).is_ok());
  assert_eq!(set.verify_token(&token, SERVER, last + 1), Err(IdentityError::Expired));
  assert_eq!(
    set.verify_token(&token, SERVER, NOW - CLOCK_SKEW_SECS - 1),
    Err(IdentityError::NotYetValid)
  );
}

#[test]
fn identity_token_for_another_server_is_refused() {
  let root = root();
  let key = signing_key();
  let set = key_set(&root, vec![listed(&key, NOW - DAY)], NOW);
  assert_eq!(
    set.verify_token(&token(&key, SERVER, NOW), "0199f0a0-0000-7000-8000-000000000002", NOW),
    Err(IdentityError::WrongAudience)
  );
}

#[test]
fn identity_forged_token_is_refused() {
  let root = root();
  let key = signing_key();
  let set = key_set(&root, vec![listed(&key, NOW - DAY)], NOW);
  let genuine = token(&key, SERVER, NOW);
  let mut parts: Vec<String> = genuine.split('.').map(Into::into).collect();
  let mut claims: serde_json::Value =
    serde_json::from_slice(&URL_SAFE_NO_PAD.decode(&parts[1]).expect("payload")).expect("claims");
  claims["sub"] = serde_json::Value::String(CloudAccountId::generate().to_string());
  parts[1] = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&claims).expect("forged claims"));
  assert_eq!(
    set.verify_token(&parts.join("."), SERVER, NOW),
    Err(IdentityError::BadSignature)
  );
  let mut signature = URL_SAFE_NO_PAD
    .decode(genuine.rsplit('.').next().expect("signature"))
    .expect("bytes");
  signature[0] ^= 1;
  let flipped = format!(
    "{}.{}",
    genuine.rsplit_once('.').expect("signed").0,
    URL_SAFE_NO_PAD.encode(signature)
  );
  assert_eq!(
    set.verify_token(&flipped, SERVER, NOW),
    Err(IdentityError::BadSignature)
  );
  let none = format!(
    "{}.{}.",
    URL_SAFE_NO_PAD.encode(format!(r#"{{"alg":"none","typ":"JWT","kid":"{}"}}"#, key.kid())),
    genuine.split('.').nth(1).expect("payload")
  );
  assert_eq!(
    set.verify_token(&none, SERVER, NOW),
    Err(IdentityError::UnsupportedAlgorithm)
  );
}

#[test]
fn identity_rotated_out_key_is_refused() {
  let root = root();
  let old = signing_key();
  let new = signing_key();
  let issued = token(&old, SERVER, NOW);
  let rotated = key_set(&root, vec![listed(&new, NOW - DAY)], NOW);
  assert_eq!(
    rotated.verify_token(&issued, SERVER, NOW),
    Err(IdentityError::UnknownKey)
  );
  let both = key_set(&root, vec![listed(&old, NOW - DAY), listed(&new, NOW)], NOW);
  assert!(
    both.verify_token(&issued, SERVER, NOW).is_ok(),
    "the previous key still verifies"
  );
  let mut ended = listed(&old, NOW - DAY);
  ended.signs_until = NOW;
  ended.listed_until = NOW;
  let ended = key_set(&root, vec![ended], NOW - 10);
  assert_eq!(ended.verify_token(&issued, SERVER, NOW), Err(IdentityError::UnknownKey));
}

#[test]
fn identity_token_from_a_key_outside_its_signing_window_is_refused() {
  let root = root();
  let key = signing_key();
  let set = key_set(&root, vec![listed(&key, NOW + 60)], NOW);
  assert_eq!(
    set.verify_token(&token(&key, SERVER, NOW), SERVER, NOW),
    Err(IdentityError::KeyNotValidAtTime)
  );
}

#[test]
fn identity_key_set_needs_the_pinned_root_and_expires() {
  let root = root();
  let key = signing_key();
  let statement = KeySetStatement::new(ISSUER.into(), vec![listed(&key, NOW - DAY)], NOW);
  let compact = root.sign_key_set(&statement).expect("sign");
  let stranger = self::root();
  assert_eq!(
    VerifiedKeySet::verify(&compact, &stranger.public_key(), NOW).map(|_| ()),
    Err(IdentityError::BadSignature)
  );
  assert_eq!(
    VerifiedKeySet::verify(&compact, &root.public_key(), statement.expires_at).map(|_| ()),
    Err(IdentityError::KeySetExpired)
  );
  let mut wrong = statement;
  wrong.keys[0].kid = signing_key().kid().into();
  assert_eq!(root.sign_key_set(&wrong), Err(IdentityError::InvalidStatement));
}

#[test]
fn identity_debug_never_shows_private_keys() {
  let private = PrivateKey::generate().expect("key");
  let document = URL_SAFE_NO_PAD.encode(private.pkcs8());
  let key = SigningKey::new(&private).expect("signing key");
  let root = RootKey::new(&private).expect("root key");
  let debug = format!("{private:?} {key:?} {root:?}");
  assert!(!debug.contains(&document));
  assert!(!debug.contains(&format!("{:?}", private.pkcs8())));
  assert!(debug.contains(key.kid()));
}
