//! Compact JWS with Ed25519 (`EdDSA`) for what the server signs for Orchester Cloud: its link statement and invites.
//! Verifying what the cloud signs is [`super::identity`]'s.

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use ring::{
  digest::{SHA256, digest},
  signature::{Ed25519KeyPair, KeyPair},
};
use serde::Serialize;

#[derive(Serialize)]
struct Header<'a> {
  alg: &'a str,
  typ: &'a str,
  kid: String,
}

/// `payload` signed with `key` as a compact JWS of the type `typ`, its header naming the key's id.
pub(crate) fn sign(key: &Ed25519KeyPair, typ: &str, payload: &impl Serialize) -> String {
  let header = Header {
    alg: "EdDSA",
    typ,
    kid: encode(&digest(&SHA256, key.public_key().as_ref()).as_ref()[..16]),
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
