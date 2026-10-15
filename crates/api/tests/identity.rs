//! Orchester Cloud's identity tokens and key sets over explicit keys and times (`identity` feature).

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use orchester_api::{
  cloud::{CloudPlatform, DeviceInfo},
  identity::{
    CLOCK_SKEW_SECS, IdentityClaims, IdentityError, KeySetStatement, ListedKey, PrivateKey, RootKey,
    STATEMENT_FRESH_SECS, SigningKey, TOKEN_LIFETIME_SECS, TokenSubject, VerifiedKeySet,
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
    .sign_key_set(&KeySetStatement::new(ISSUER.into(), keys, issued_at), issued_at)
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
      invite: None,
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
    .expect("valid token")
    .claims;
  assert_eq!(claims.aud, SERVER);
  assert_eq!(claims.iss, ISSUER);
  assert_eq!(claims.exp - claims.iat, TOKEN_LIFETIME_SECS);
  assert_eq!(claims.device.platform, CloudPlatform::Macos);
  let other = set
    .verify_token(&token(&key, SERVER, NOW), SERVER, NOW)
    .expect("second token")
    .claims;
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
  let compact = root.sign_key_set(&statement, NOW).expect("sign");
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
  assert_eq!(root.sign_key_set(&wrong, NOW), Err(IdentityError::InvalidStatement));
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

/// Review 1, M3: a token is refused once the key set it is checked against has expired, even when that key set was
/// verified while it was valid.
#[test]
fn identity_token_is_refused_once_the_key_set_expired() {
  let root = root();
  let key = signing_key();
  let set = key_set(&root, vec![listed(&key, NOW)], NOW);
  let expires = set.statement().expires_at;
  let late = token(&key, SERVER, expires - 10);
  assert!(set.verify_token(&late, SERVER, expires - 1).is_ok());
  assert_eq!(
    set.verify_token(&late, SERVER, expires),
    Err(IdentityError::KeySetExpired)
  );
}

/// Review 1, M2: a key set whose lifetime or key windows are longer than the rules allow is refused, even when the
/// root signed it.
#[test]
fn identity_key_set_with_overlong_lifetimes_is_refused() {
  let private = PrivateKey::generate().expect("root key");
  let root = RootKey::new(&private).expect("root");
  let key = signing_key();
  let mut long_set = KeySetStatement::new(ISSUER.into(), vec![listed(&key, NOW - DAY)], NOW);
  long_set.expires_at = NOW + 365 * DAY;
  let mut long_key = KeySetStatement::new(ISSUER.into(), vec![listed(&key, NOW - DAY)], NOW);
  long_key.keys[0].signs_until = NOW + 365 * DAY;
  long_key.keys[0].listed_until = NOW + 400 * DAY;
  for statement in [long_set, long_key] {
    assert_eq!(root.sign_key_set(&statement, NOW), Err(IdentityError::InvalidStatement));
    let compact = signed_by_hand(&private, &statement);
    assert!(
      VerifiedKeySet::verify(&compact, &root.public_key(), NOW).is_err(),
      "an overlong key set must not verify"
    );
  }
}

/// Review 1, M2: the root signs only a statement prepared within the last day.
#[test]
fn identity_root_signs_only_a_fresh_statement() {
  let root = root();
  let key = signing_key();
  let statement = KeySetStatement::new(ISSUER.into(), vec![listed(&key, NOW - DAY)], NOW);
  assert!(root.sign_key_set(&statement, NOW + STATEMENT_FRESH_SECS).is_ok());
  assert_eq!(
    root.sign_key_set(&statement, NOW + STATEMENT_FRESH_SECS + 1),
    Err(IdentityError::StaleStatement)
  );
  assert_eq!(
    root.sign_key_set(&statement, NOW - CLOCK_SKEW_SECS - 1),
    Err(IdentityError::StaleStatement)
  );
}

/// Signs a statement with the root's raw key, past `RootKey::sign_key_set`'s own checks, as a careless or stolen
/// signer could.
fn signed_by_hand(root: &PrivateKey, statement: &KeySetStatement) -> String {
  let pair = ring::signature::Ed25519KeyPair::from_pkcs8(root.pkcs8()).expect("root pair");
  let header = URL_SAFE_NO_PAD.encode(br#"{"alg":"EdDSA","typ":"orchester-key-set+jws"}"#);
  let payload = URL_SAFE_NO_PAD.encode(serde_json::to_vec(statement).expect("statement"));
  let input = format!("{header}.{payload}");
  let signature = URL_SAFE_NO_PAD.encode(pair.sign(input.as_bytes()).as_ref());
  format!("{input}.{signature}")
}

/// Review 2: a hand-signed key set with times at the ends of the range is refused, not a panic or a wrapped window.
#[test]
fn identity_key_set_with_extreme_times_is_refused() {
  let private = PrivateKey::generate().expect("root key");
  let root = RootKey::new(&private).expect("root");
  let key = signing_key();
  let mut wide = KeySetStatement::new(ISSUER.into(), vec![listed(&key, NOW)], NOW);
  wide.issued_at = i64::MIN;
  wide.expires_at = i64::MAX;
  let mut key_window = KeySetStatement::new(ISSUER.into(), vec![listed(&key, NOW)], NOW);
  key_window.keys[0].not_before = i64::MIN;
  key_window.keys[0].signs_until = i64::MAX;
  for statement in [wide, key_window] {
    let compact = signed_by_hand(&private, &statement);
    assert!(VerifiedKeySet::verify(&compact, &root.public_key(), NOW).is_err());
  }
}

/// A team server's statement verifies with the key that signed it and with no other; its type must match, and reading
/// it unverified is only for finding the key a link statement names.
#[test]
fn identity_server_statements_verify_with_their_key_and_type() {
  use orchester_api::{
    cloud::{INVITE_STATEMENT_TYPE, LINK_STATEMENT_TYPE, LinkStatement},
    identity::{unverified_statement, verify_statement},
  };
  let server = signing_key();
  let statement = LinkStatement {
    server: SERVER.parse().expect("a server id"),
    name: "Kontur team".into(),
    address: "https://team.example.test".into(),
    public_key: server.public_key().encoded(),
    fingerprint: None,
    issued_at: NOW,
  };
  let signed = server.sign_statement(LINK_STATEMENT_TYPE, &statement).expect("sign");
  let named: LinkStatement = unverified_statement(&signed, LINK_STATEMENT_TYPE).expect("readable");
  assert_eq!(named, statement);
  let verified: LinkStatement = verify_statement(&signed, LINK_STATEMENT_TYPE, &server.public_key()).expect("verifies");
  assert_eq!(verified, statement);
  let other = signing_key().public_key();
  assert_eq!(
    verify_statement::<LinkStatement>(&signed, LINK_STATEMENT_TYPE, &other),
    Err(IdentityError::BadSignature)
  );
  assert_eq!(
    verify_statement::<LinkStatement>(&signed, INVITE_STATEMENT_TYPE, &server.public_key()),
    Err(IdentityError::Malformed)
  );
}
