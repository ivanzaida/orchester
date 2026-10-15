//! A test Orchester Cloud on `orchester-api`'s `identity` keys: its root key, signing keys, key sets and identity
//! tokens, over explicit times, with no network.

use orchester_api::{
  cloud::{CloudPlatform, DeviceInfo},
  identity::{IdentityClaims, KeySetStatement, ListedKey, PrivateKey, RootKey, SigningKey},
};
use orchester_core::id::IdempotencyKey;

pub const ISSUER: &str = "https://cloud.test";
pub const DAY: i64 = 24 * 60 * 60;

/// An Ed25519 key, used as the cloud's root or as one of its signing keys.
pub struct Key {
  private: PrivateKey,
}

impl Key {
  pub fn generate() -> Self {
    Self {
      private: PrivateKey::generate().expect("test key"),
    }
  }

  fn signing(&self) -> SigningKey {
    SigningKey::new(&self.private).expect("signing key")
  }

  pub fn public(&self) -> String {
    self.signing().public_key().encoded()
  }

  pub fn kid(&self) -> String {
    self.signing().kid().to_owned()
  }

  pub fn fingerprint(&self) -> String {
    self.signing().public_key().fingerprint()
  }
}

/// What a token says; `TestCloud::token` fills the rest. `subject` is a cloud account id.
pub struct Identity<'a> {
  pub audience: &'a str,
  pub subject: &'a str,
  pub email: &'a str,
  pub name: &'a str,
  pub invite: Option<&'a str>,
}

pub struct TestCloud {
  pub root: Key,
  pub signing: Key,
}

impl TestCloud {
  pub fn generate() -> Self {
    Self {
      root: Key::generate(),
      signing: Key::generate(),
    }
  }

  /// A key set issued at `issued_at` (seconds) that lists `keys`, each signing for 30 days from the day before
  /// `issued_at` and listed 60 more, signed by `root`.
  pub fn key_set_by(root: &Key, keys: &[&Key], issued_at: i64) -> String {
    let listed = keys
      .iter()
      .map(|key| ListedKey {
        kid: key.kid(),
        public_key: key.public(),
        not_before: issued_at - DAY,
        signs_until: issued_at + 29 * DAY,
        listed_until: issued_at + 89 * DAY,
      })
      .collect();
    let statement = KeySetStatement::new(ISSUER.into(), listed, issued_at);
    RootKey::new(&root.private)
      .expect("root key")
      .sign_key_set(&statement, issued_at)
      .expect("key set")
  }

  pub fn key_set(&self, issued_at: i64) -> String {
    Self::key_set_by(&self.root, &[&self.signing], issued_at)
  }

  /// An identity token signed by `key`, issued at `iat` (seconds), valid five minutes.
  pub fn token_by(key: &Key, identity: &Identity<'_>, iat: i64) -> String {
    let claims = IdentityClaims {
      iss: ISSUER.into(),
      aud: identity.audience.into(),
      sub: identity.subject.parse().expect("a cloud account id"),
      email: identity.email.into(),
      name: identity.name.into(),
      device: DeviceInfo {
        name: "Test computer".into(),
        platform: CloudPlatform::Windows,
      },
      invite: identity.invite.map(Into::into),
      jti: IdempotencyKey::generate().to_string(),
      iat,
      exp: iat + 300,
    };
    key.signing().sign_token(&claims).expect("token").expose().to_owned()
  }

  pub fn token(&self, identity: &Identity<'_>, iat: i64) -> String {
    Self::token_by(&self.signing, identity, iat)
  }
}
