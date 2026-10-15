//! A test Orchester Cloud: its root key, signing keys, key sets and identity tokens in the format the cloud signs
//! (`docs/architecture/server.md`, "Identity tokens and keys"), over explicit times, with no network.

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use ring::{
  digest::{SHA256, digest},
  rand::SystemRandom,
  signature::{Ed25519KeyPair, KeyPair},
};
use serde_json::{Value, json};

pub const ISSUER: &str = "https://cloud.test";
pub const DAY: i64 = 24 * 60 * 60;

pub struct Key {
  pair: Ed25519KeyPair,
}

impl Key {
  pub fn generate() -> Self {
    let document = Ed25519KeyPair::generate_pkcs8(&SystemRandom::new()).expect("test key");
    Self {
      pair: Ed25519KeyPair::from_pkcs8(document.as_ref()).expect("test key pair"),
    }
  }

  pub fn public(&self) -> String {
    URL_SAFE_NO_PAD.encode(self.pair.public_key().as_ref())
  }

  pub fn kid(&self) -> String {
    URL_SAFE_NO_PAD.encode(&digest(&SHA256, self.pair.public_key().as_ref()).as_ref()[..16])
  }

  pub fn fingerprint(&self) -> String {
    URL_SAFE_NO_PAD.encode(digest(&SHA256, self.pair.public_key().as_ref()).as_ref())
  }

  fn sign(&self, typ: &str, kid: Option<String>, payload: &Value) -> String {
    let mut header = json!({"alg":"EdDSA","typ":typ});
    if let Some(kid) = kid {
      header["kid"] = json!(kid);
    }
    let input = format!(
      "{}.{}",
      URL_SAFE_NO_PAD.encode(header.to_string()),
      URL_SAFE_NO_PAD.encode(payload.to_string())
    );
    let signature = self.pair.sign(input.as_bytes());
    format!("{input}.{}", URL_SAFE_NO_PAD.encode(signature.as_ref()))
  }
}

/// What a token says; `TestCloud::token` fills the rest.
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
    let listed: Vec<Value> = keys
      .iter()
      .map(|key| {
        json!({"kid": key.kid(), "public_key": key.public(), "not_before": issued_at - DAY,
          "signs_until": issued_at + 29 * DAY, "listed_until": issued_at + 89 * DAY})
      })
      .collect();
    let statement = json!({"iss": ISSUER, "issued_at": issued_at, "expires_at": issued_at + 30 * DAY, "keys": listed});
    Self::statement_by(root, &statement)
  }

  /// Any key set statement, signed by `root`: for statements the cloud would never sign.
  pub fn statement_by(root: &Key, statement: &Value) -> String {
    root.sign("orchester-key-set+jws", None, statement)
  }

  pub fn key_set(&self, issued_at: i64) -> String {
    Self::key_set_by(&self.root, &[&self.signing], issued_at)
  }

  /// An identity token signed by `key`, issued at `iat` (seconds), valid five minutes.
  pub fn token_by(key: &Key, identity: &Identity<'_>, iat: i64) -> String {
    let jti = URL_SAFE_NO_PAD.encode(digest(
      &SHA256,
      format!("{}{iat}{}", identity.subject, rand_suffix()).as_bytes(),
    ));
    let mut claims = json!({"iss": ISSUER, "aud": identity.audience, "sub": identity.subject, "email": identity.email,
      "name": identity.name, "device": {"name": "Test computer", "platform": "windows"}, "jti": jti,
      "iat": iat, "exp": iat + 300});
    if let Some(invite) = identity.invite {
      claims["invite"] = json!(invite);
    }
    key.sign("JWT", Some(key.kid()), &claims)
  }

  pub fn token(&self, identity: &Identity<'_>, iat: i64) -> String {
    Self::token_by(&self.signing, identity, iat)
  }
}

fn rand_suffix() -> String {
  let mut bytes = [0_u8; 8];
  ring::rand::SecureRandom::fill(&SystemRandom::new(), &mut bytes).expect("random");
  URL_SAFE_NO_PAD.encode(bytes)
}
