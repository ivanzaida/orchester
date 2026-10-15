use orchester_api::Credential;
use ring::{
  digest::{SHA256, digest},
  rand::{SecureRandom, SystemRandom},
};

use crate::error::CloudError;

/// A cloud device token: 32 random bytes as `orcc_` and 64 hex characters (the team server's tokens use `orc_`).
pub(crate) const DEVICE_PREFIX: &str = "orcc_";

pub(crate) fn random_bytes<const N: usize>() -> Result<[u8; N], CloudError> {
  let mut bytes = [0u8; N];
  SystemRandom::new().fill(&mut bytes).map_err(|_| CloudError::Random)?;
  Ok(bytes)
}

/// A device token or a link token: 32 random bytes in hex, after `prefix`.
pub(crate) fn random_token(prefix: &str) -> Result<Credential, CloudError> {
  const HEX: &[u8; 16] = b"0123456789abcdef";
  let mut value = String::with_capacity(prefix.len() + 64);
  value.push_str(prefix);
  for byte in random_bytes::<32>()? {
    value.push(HEX[usize::from(byte >> 4)] as char);
    value.push(HEX[usize::from(byte & 15)] as char);
  }
  Ok(Credential::new(value))
}

/// The cloud keeps only this SHA-256 of a token; a lookup by it reveals nothing about the token.
pub(crate) fn token_hash(token: &Credential) -> Vec<u8> {
  digest(&SHA256, token.expose().as_bytes()).as_ref().to_vec()
}

/// `prefix` followed by exactly 64 lowercase hex characters.
pub(crate) fn well_formed(token: &str, prefix: &str) -> bool {
  token.len() == prefix.len() + 64
    && token.starts_with(prefix)
    && token[prefix.len()..]
      .bytes()
      .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}
