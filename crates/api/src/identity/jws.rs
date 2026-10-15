use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use super::IdentityError;

/// Tokens and key sets are small; anything larger is refused before decoding.
const MAX_COMPACT: usize = 16 * 1024;

#[derive(Serialize, Deserialize)]
pub(super) struct Header {
  pub alg: String,
  pub typ: String,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub kid: Option<String>,
}

impl Header {
  pub fn new(typ: &str, kid: Option<&str>) -> Self {
    Self {
      alg: "EdDSA".into(),
      typ: typ.into(),
      kid: kid.map(Into::into),
    }
  }
}

pub(super) struct Compact {
  pub header: Header,
  pub signing_input: Vec<u8>,
  pub payload: Vec<u8>,
  pub signature: Vec<u8>,
}

pub(super) fn signing_input(header: &Header, payload: &impl Serialize) -> Result<String, IdentityError> {
  let header = serde_json::to_vec(header).map_err(|_| IdentityError::Malformed)?;
  let payload = serde_json::to_vec(payload).map_err(|_| IdentityError::Malformed)?;
  Ok(format!("{}.{}", encode(&header), encode(&payload)))
}

pub(super) fn finish(signing_input: &str, signature: &[u8]) -> String {
  format!("{signing_input}.{}", encode(signature))
}

pub(super) fn decode(text: &str, typ: &str) -> Result<Compact, IdentityError> {
  if text.len() > MAX_COMPACT {
    return Err(IdentityError::Malformed);
  }
  let mut parts = text.split('.');
  let (Some(header), Some(payload), Some(signature), None) = (parts.next(), parts.next(), parts.next(), parts.next())
  else {
    return Err(IdentityError::Malformed);
  };
  let header_json = decode_part(header)?;
  let header: Header = serde_json::from_slice(&header_json).map_err(|_| IdentityError::Malformed)?;
  if header.alg != "EdDSA" {
    return Err(IdentityError::UnsupportedAlgorithm);
  }
  if header.typ != typ {
    return Err(IdentityError::Malformed);
  }
  Ok(Compact {
    header,
    signing_input: text.as_bytes()[..text.len() - signature.len() - 1].to_vec(),
    payload: decode_part(payload)?,
    signature: decode_part(signature)?,
  })
}

pub(super) fn parse<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, IdentityError> {
  serde_json::from_slice(bytes).map_err(|_| IdentityError::Malformed)
}

pub(super) fn encode(bytes: &[u8]) -> String {
  URL_SAFE_NO_PAD.encode(bytes)
}

pub(super) fn decode_part(text: &str) -> Result<Vec<u8>, IdentityError> {
  URL_SAFE_NO_PAD.decode(text).map_err(|_| IdentityError::Malformed)
}
