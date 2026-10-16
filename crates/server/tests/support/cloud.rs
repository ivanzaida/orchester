//! A test Orchester Cloud on `orchester-api`'s `identity` keys: its root key, signing keys, key sets and identity
//! tokens, over explicit times, with no network.

use orchester_api::{
  cloud::{CloudPlatform, DeviceInfo, TokenPurpose},
  identity::{IdentityClaims, KeySetStatement, ListedKey, PrivateKey, RootKey, SigningKey},
};
use orchester_core::id::IdempotencyKey;

pub const ISSUER: &str = "https://cloud.test";
pub const DAY: i64 = 24 * 60 * 60;

/// An Ed25519 key, used as the cloud's root or as one of its signing keys.
pub struct Key {
  private: PrivateKey,
}

impl Clone for Key {
  fn clone(&self) -> Self {
    Self {
      private: PrivateKey::from_pkcs8(self.private.pkcs8().to_vec()),
    }
  }
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

#[derive(Clone)]
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

  /// An identity token signed by `key`, issued at `iat` (seconds), valid five minutes. One that accepts an invite says
  /// its device signed in with two-factor authentication, as the cloud issues no other.
  pub fn token_by(key: &Key, identity: &Identity<'_>, iat: i64) -> String {
    Self::token_with(key, identity, (None, identity.invite.is_some()), iat)
  }

  /// An identity token for one use besides joining: the server admin, or linking.
  pub fn purposed_token(&self, identity: &Identity<'_>, purpose: TokenPurpose, iat: i64) -> String {
    Self::token_with(&self.signing, identity, (Some(purpose), false), iat)
  }

  /// A token for the server admin from a cloud device that signed in with two-factor authentication.
  pub fn two_factor_admin_token(&self, identity: &Identity<'_>, iat: i64) -> String {
    Self::token_with(&self.signing, identity, (Some(TokenPurpose::Admin), true), iat)
  }

  /// A join token whose two-factor claim says `mfa`, whatever it accepts.
  pub fn token_mfa(&self, identity: &Identity<'_>, mfa: bool, iat: i64) -> String {
    Self::token_with(&self.signing, identity, (None, mfa), iat)
  }

  /// A token for codes (purpose `code`) whose two-factor claim says `mfa`, bound to the certificate with the
  /// fingerprint `certificate` (hex) when there is one, as the cloud binds it for a server it has not registered.
  pub fn code_token(&self, identity: &Identity<'_>, (mfa, certificate): (bool, Option<&str>), iat: i64) -> String {
    let mut claims = Self::claims(identity, (Some(TokenPurpose::Code), mfa), iat);
    claims.certificate = certificate.map(str::to_owned);
    self
      .signing
      .signing()
      .sign_token(&claims)
      .expect("token")
      .expose()
      .to_owned()
  }

  fn token_with(key: &Key, identity: &Identity<'_>, (purpose, mfa): (Option<TokenPurpose>, bool), iat: i64) -> String {
    let claims = Self::claims(identity, (purpose, mfa), iat);
    key.signing().sign_token(&claims).expect("token").expose().to_owned()
  }

  fn claims(identity: &Identity<'_>, (purpose, mfa): (Option<TokenPurpose>, bool), iat: i64) -> IdentityClaims {
    IdentityClaims {
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
      purpose,
      mfa,
      certificate: None,
      jti: IdempotencyKey::generate().to_string(),
      iat,
      exp: iat + 300,
    }
  }

  pub fn token(&self, identity: &Identity<'_>, iat: i64) -> String {
    Self::token_by(&self.signing, identity, iat)
  }
}

/// The cloud account id of the test person `name`: every person their own, the same in every test.
pub fn person(name: &str) -> String {
  let hash = name.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
    (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
  });
  format!("0191c7c0-0000-7000-8000-{:012x}", hash & 0xffff_ffff_ffff)
}
