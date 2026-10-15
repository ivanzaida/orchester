//! Verifying Orchester Cloud's key sets and identity tokens locally, with no request to the cloud
//! (`docs/architecture/server.md`, "Identity tokens and keys").
//!
//! Everything the server knows about verifying what the cloud signs is in this module, so it is the one module that
//! changes when `orchester-api`'s `identity` feature (branch `claude/cloud-auth`) is on master: `RootKey` becomes its
//! `PublicKey` (`parse`, `encoded`, `fingerprint`), `verify_key_set` its `VerifiedKeySet::verify` (`KeySet`'s
//! `issued_at` and `expires_at` from `statement()`), `verify_token` its `verify_token`, and `Claims` its
//! `IdentityClaims` (`sub` a `CloudAccountId`; the signing key's id, which it does not return, read from the token's
//! header). The format and rules here are that module's: compact JWS `EdDSA`, key set type `orchester-key-set+jws`,
//! identity token type `JWT`, keys named by an id derived from the key, 2 minutes of clock skew, 5-minute tokens, a key
//! set living at most 30 days and a key signing at most 30 days and listed at most 60 more.

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use ring::{
  digest::{SHA256, digest},
  signature::{ED25519, UnparsedPublicKey},
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

/// The clock difference between the cloud and this server that verification tolerates, in seconds.
pub(crate) const CLOCK_SKEW_SECS: i64 = 2 * 60;
/// An identity token is valid for at most five minutes after it was issued.
const TOKEN_LIFETIME_SECS: i64 = 5 * 60;
const DAY_SECS: i64 = 24 * 60 * 60;
/// A key set lives at most 30 days; a key signs for at most 30 days and stays listed at most 60 more. A verifier
/// checks it too, so even a careless or stolen signer cannot make a long-lived key set this server believes.
const KEY_SET_LIFETIME_SECS: i64 = 30 * DAY_SECS;
const KEY_SIGNS_SECS: i64 = 30 * DAY_SECS;
const KEY_LISTED_AFTER_SECS: i64 = 60 * DAY_SECS;
const KEY_SET_TYPE: &str = "orchester-key-set+jws";
const TOKEN_TYPE: &str = "JWT";
/// Tokens and key sets are small; anything larger is refused before decoding.
const MAX_COMPACT: usize = 16 * 1024;

/// Why a key set or token was refused. No variant carries the token, a key or a claim value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum IdentityError {
  /// Malformed, forged or inconsistent.
  Invalid,
  UnknownKey,
  WrongIssuer,
  WrongAudience,
  Expired,
  NotYetValid,
  KeySetExpired,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ListedKey {
  pub kid: String,
  pub public_key: String,
  pub not_before: i64,
  pub signs_until: i64,
  pub listed_until: i64,
}

/// A key set the pinned root signed. Times are seconds since the Unix epoch.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct KeySet {
  pub iss: String,
  pub issued_at: i64,
  pub expires_at: i64,
  pub keys: Vec<ListedKey>,
}

/// What a verified identity token says that the server uses; the token's `device` is the cloud's record, the request
/// names the device the server issues its token to.
#[derive(Clone, Debug, Deserialize)]
pub(crate) struct Claims {
  pub iss: String,
  pub aud: String,
  /// The cloud account.
  pub sub: String,
  /// The account's confirmed email.
  pub email: String,
  pub name: String,
  /// The invite the token was issued for, when it came from accepting one.
  #[serde(default)]
  pub invite: Option<String>,
  pub jti: String,
  pub iat: i64,
  pub exp: i64,
  /// The id of the cloud key that signed the token, from its header.
  #[serde(skip)]
  pub kid: String,
}

/// The cloud's root key the server pins: an Ed25519 public key, written as 32 bytes of base64url.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct RootKey([u8; 32]);

impl RootKey {
  pub fn parse(text: &str) -> Option<Self> {
    public_key(text).map(Self)
  }

  pub fn encoded(&self) -> String {
    URL_SAFE_NO_PAD.encode(self.0)
  }

  /// The base64url SHA-256 of the key: what owners compare.
  pub fn fingerprint(&self) -> String {
    URL_SAFE_NO_PAD.encode(digest(&SHA256, &self.0).as_ref())
  }
}

/// The key set in `compact`, when the root key `root` signed it, its keys are consistent and it has not expired.
pub(crate) fn verify_key_set(compact: &str, root: &RootKey, now: i64) -> Result<KeySet, IdentityError> {
  let set: KeySet = Compact::decode(compact, KEY_SET_TYPE)
    .and_then(|compact| compact.verified(&root.0))
    .ok_or(IdentityError::Invalid)?;
  let lifetime = set.expires_at - set.issued_at;
  let consistent = !set.keys.is_empty()
    && (1..=KEY_SET_LIFETIME_SECS).contains(&lifetime)
    && set.keys.iter().all(|listed| {
      public_key(&listed.public_key).is_some_and(|key| kid(&key) == listed.kid)
        && (1..=KEY_SIGNS_SECS).contains(&(listed.signs_until - listed.not_before))
        && (0..=KEY_LISTED_AFTER_SECS).contains(&(listed.listed_until - listed.signs_until))
    });
  if !consistent {
    return Err(IdentityError::Invalid);
  }
  if set.issued_at > now + CLOCK_SKEW_SECS {
    return Err(IdentityError::NotYetValid);
  }
  if now >= set.expires_at {
    return Err(IdentityError::KeySetExpired);
  }
  Ok(set)
}

/// The claims of an identity token for the team server `audience`, signed by a key `set` lists for the token's time.
pub(crate) fn verify_token(set: &KeySet, token: &str, audience: &str, now: i64) -> Result<Claims, IdentityError> {
  let compact = Compact::decode(token, TOKEN_TYPE).ok_or(IdentityError::Invalid)?;
  let kid = compact.header.kid.as_deref().ok_or(IdentityError::Invalid)?;
  let listed = set
    .keys
    .iter()
    .find(|listed| listed.kid == kid && now < listed.listed_until)
    .ok_or(IdentityError::UnknownKey)?;
  let key = public_key(&listed.public_key).ok_or(IdentityError::Invalid)?;
  let mut claims: Claims = compact.verified(&key).ok_or(IdentityError::Invalid)?;
  claims.kid = listed.kid.clone();
  if claims.iss != set.iss {
    return Err(IdentityError::WrongIssuer);
  }
  if claims.aud != audience {
    return Err(IdentityError::WrongAudience);
  }
  if claims.exp <= claims.iat || claims.exp - claims.iat > TOKEN_LIFETIME_SECS {
    return Err(IdentityError::Invalid);
  }
  if claims.iat < listed.not_before || claims.iat >= listed.signs_until {
    return Err(IdentityError::UnknownKey);
  }
  if claims.iat > now + CLOCK_SKEW_SECS {
    return Err(IdentityError::NotYetValid);
  }
  if now >= claims.exp + CLOCK_SKEW_SECS {
    return Err(IdentityError::Expired);
  }
  Ok(claims)
}

/// A decoded compact JWS whose signature is not checked yet.
struct Compact {
  header: Header,
  signing_input: Vec<u8>,
  payload: Vec<u8>,
  signature: Vec<u8>,
}

#[derive(Deserialize)]
struct Header {
  alg: String,
  typ: String,
  #[serde(default)]
  kid: Option<String>,
}

impl Compact {
  /// Decodes `text` of the type `typ`; `None` for anything else.
  fn decode(text: &str, typ: &str) -> Option<Self> {
    if text.len() > MAX_COMPACT {
      return None;
    }
    let mut parts = text.split('.');
    let (Some(header), Some(payload), Some(signature), None) = (parts.next(), parts.next(), parts.next(), parts.next())
    else {
      return None;
    };
    let header: Header = serde_json::from_slice(&decode(header)?).ok()?;
    if header.alg != "EdDSA" || header.typ != typ {
      return None;
    }
    Some(Self {
      header,
      signing_input: text.as_bytes()[..text.len() - signature.len() - 1].to_vec(),
      payload: decode(payload)?,
      signature: decode(signature)?,
    })
  }

  /// The payload, once the signature verifies with the Ed25519 `public_key`.
  fn verified<T: DeserializeOwned>(&self, public_key: &[u8; 32]) -> Option<T> {
    UnparsedPublicKey::new(&ED25519, public_key)
      .verify(&self.signing_input, &self.signature)
      .ok()?;
    serde_json::from_slice(&self.payload).ok()
  }
}

fn decode(text: &str) -> Option<Vec<u8>> {
  URL_SAFE_NO_PAD.decode(text).ok()
}

fn public_key(text: &str) -> Option<[u8; 32]> {
  decode(text.trim())?.try_into().ok()
}

/// A key's id, derived from the key: the base64url of the first 16 bytes of its SHA-256.
fn kid(public_key: &[u8; 32]) -> String {
  URL_SAFE_NO_PAD.encode(&digest(&SHA256, public_key).as_ref()[..16])
}
