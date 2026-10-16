//! Orchester Cloud's keys in the tests: one fixed root and one fixed signing key that every stand-in cloud signs with
//! (`sharing_support::{cloud, account_cloud}`, and the test server's own code joins), so a team server pins one root
//! whichever stand-in a test uses, and a code entered in an app carries a token that root vouches for. Fixed bytes,
//! not generated ones: each test binary includes this file under more than one module.

#![allow(dead_code, reason = "shared by test binaries, each of which uses a part")]

use std::time::{SystemTime, UNIX_EPOCH};

use orchester_api::{
  cloud::{CloudPlatform, DeviceInfo, TokenPurpose},
  identity::{IdentityClaims, KeySetStatement, ListedKey, PrivateKey, RootKey, SigningKey},
};
use orchester_core::id::IdempotencyKey;

pub const ISSUER: &str = "https://cloud.test";
const DAY: i64 = 24 * 60 * 60;
/// Ed25519 PKCS#8 documents made for these tests only; they sign nothing outside them.
const ROOT: &str = "3053020101300506032b657004220420adfab66f382aaa72a53a8960fee4a52fd00b01bde82e007de7d6259e3a2543d0a12303210052ee3b44c98237fedeb12e5ab420d43fdfe90383bb571607cc41b66eaf272c3e";
const SIGNING: &str = "3053020101300506032b65700422042096a1366df61a8a963045c82480e9ff10671dd04653fc8f5b46fe763f8f2ffc28a12303210086e961cfbd6dd6506beba7150c72841a453d6be4d129ab870dc2f56e01def44d";

fn private(hex: &str) -> PrivateKey {
  let bytes = (0..hex.len())
    .step_by(2)
    .map(|index| u8::from_str_radix(&hex[index..index + 2], 16).expect("hex"))
    .collect();
  PrivateKey::from_pkcs8(bytes)
}

pub fn root() -> RootKey {
  RootKey::new(&private(ROOT)).expect("root key")
}

pub fn signing() -> SigningKey {
  SigningKey::new(&private(SIGNING)).expect("signing key")
}

/// The root's public key, which a team server pins.
pub fn root_key() -> String {
  root().public_key().encoded()
}

/// A key set issued at `now` (seconds), listing the signing key from the day before for 30 days.
pub fn key_set(now: i64) -> String {
  let signing = signing();
  let listed = ListedKey {
    kid: signing.kid().to_owned(),
    public_key: signing.public_key().encoded(),
    not_before: now - DAY,
    signs_until: now + 29 * DAY,
    listed_until: now + 89 * DAY,
  };
  let statement = KeySetStatement::new(ISSUER.into(), vec![listed], now);
  root().sign_key_set(&statement, now).expect("key set")
}

/// The cloud account id of the test person `username`: every person their own, the same in every test, so an
/// account linked to one takes only that one.
pub fn account_of(username: &str) -> String {
  let hash = username.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
    (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
  });
  format!("0191c7c0-0000-7000-8000-{:012x}", hash & 0xffff_ffff_ffff)
}

/// A five-minute token for codes (purpose `code`) to the team server `audience`, about the test person `username`,
/// whose device signed in with two-factor authentication: what that person's app sends with a code.
pub fn code_token(audience: &str, username: &str) -> String {
  let iat = now();
  let claims = IdentityClaims {
    iss: ISSUER.into(),
    aud: audience.into(),
    sub: account_of(username).parse().expect("a cloud account id"),
    email: format!("{username}@example.com"),
    name: username.into(),
    device: DeviceInfo {
      name: "Test computer".into(),
      platform: CloudPlatform::Windows,
    },
    invite: None,
    purpose: Some(TokenPurpose::Code),
    mfa: true,
    certificate: None,
    jti: IdempotencyKey::generate().to_string(),
    iat,
    exp: iat + 300,
  };
  signing().sign_token(&claims).expect("token").expose().to_owned()
}

pub fn now() -> i64 {
  let elapsed = SystemTime::now().duration_since(UNIX_EPOCH).expect("after 1970");
  i64::try_from(elapsed.as_secs()).expect("seconds fit")
}
