//! Compact JWS with Ed25519 (`EdDSA`): the form of the cloud's identity tokens and key sets, and of what the server
//! signs for the cloud (its link statement and invites).

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use ring::{
  digest::{SHA256, digest},
  signature::{ED25519, Ed25519KeyPair, KeyPair, UnparsedPublicKey},
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

/// Tokens and key sets are small; anything larger is refused before decoding.
const MAX_COMPACT: usize = 16 * 1024;

#[derive(Serialize, Deserialize)]
pub(crate) struct Header {
  pub alg: String,
  pub typ: String,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub kid: Option<String>,
}

/// A decoded compact JWS whose signature is not checked yet.
pub(crate) struct Compact {
  pub header: Header,
  signing_input: Vec<u8>,
  payload: Vec<u8>,
  signature: Vec<u8>,
}

impl Compact {
  /// Decodes `text` of the type `typ`; `None` for anything else.
  pub fn decode(text: &str, typ: &str) -> Option<Self> {
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
  pub fn verified<T: DeserializeOwned>(&self, public_key: &[u8; 32]) -> Option<T> {
    UnparsedPublicKey::new(&ED25519, public_key)
      .verify(&self.signing_input, &self.signature)
      .ok()?;
    serde_json::from_slice(&self.payload).ok()
  }
}

/// `payload` signed with `key` as a compact JWS of the type `typ`, its header naming the key's id.
pub(crate) fn sign(key: &Ed25519KeyPair, typ: &str, payload: &impl Serialize) -> String {
  let public: [u8; 32] = key
    .public_key()
    .as_ref()
    .try_into()
    .expect("an Ed25519 public key is 32 bytes");
  let header = Header {
    alg: "EdDSA".into(),
    typ: typ.into(),
    kid: Some(kid(&public)),
  };
  let header = serde_json::to_vec(&header).expect("a JWS header serializes");
  let payload = serde_json::to_vec(payload).expect("a statement of plain fields serializes");
  let input = format!("{}.{}", encode(&header), encode(&payload));
  let signature = key.sign(input.as_bytes());
  format!("{input}.{}", encode(signature.as_ref()))
}

pub(crate) fn encode(bytes: &[u8]) -> String {
  URL_SAFE_NO_PAD.encode(bytes)
}

pub(crate) fn decode(text: &str) -> Option<Vec<u8>> {
  URL_SAFE_NO_PAD.decode(text).ok()
}

/// An Ed25519 public key written as 32 bytes of base64url.
pub(crate) fn public_key(text: &str) -> Option<[u8; 32]> {
  decode(text.trim())?.try_into().ok()
}

/// The base64url SHA-256 of a public key: what the server pins of the cloud's root key.
pub(crate) fn fingerprint(public_key: &[u8; 32]) -> String {
  encode(digest(&SHA256, public_key).as_ref())
}

/// A key's id, derived from the key: the base64url of the first 16 bytes of its SHA-256.
pub(crate) fn kid(public_key: &[u8; 32]) -> String {
  encode(&digest(&SHA256, public_key).as_ref()[..16])
}
