//! The cloud's signing keys and key sets (`docs/architecture/server.md`, "Identity tokens and keys").
//!
//! A signing key signs identity tokens for 30 days and stays listed 60 more. The offline root key signs each key set;
//! the cloud only stores what was signed (`keys prepare` on the host, `keys sign` on the offline machine, `keys
//! publish` on the host) and signs tokens only with a key the newest published, unexpired key set lists.

mod host;
mod issue;

use std::fmt;

use orchester_api::{
  Credential,
  identity::{KEY_LISTED_AFTER_SECS, KEY_SIGNS_SECS, PrivateKey},
};
use ring::aead::{AES_256_GCM, Aad, LessSafeKey, NONCE_LEN, Nonce, UnboundKey};
use zeroize::Zeroize;

use crate::{clock::DAY_MS, error::CloudError, tokens::random_bytes};

const SIGNS_MS: i64 = KEY_SIGNS_SECS * 1000;
const LISTED_AFTER_MS: i64 = KEY_LISTED_AFTER_SECS * 1000;
/// A rotated key waits this long before it signs, so team servers that refresh the key set daily have it by then
/// (review 1, L4). The first key signs at once: no team server holds a key set before it.
const OVERLAP_MS: i64 = 2 * DAY_MS;

/// Seals signing keys' private halves with the key-encryption key from the host's environment.
pub(crate) struct KeySealing(LessSafeKey);

impl KeySealing {
  /// The key-encryption key is 64 hex characters (32 bytes).
  pub fn new(key: &Credential) -> Result<Self, CloudError> {
    let invalid = || CloudError::Configuration("ORCHESTER_CLOUD_KEY_ENCRYPTION_KEY must be 64 hex characters");
    let text = key.expose().trim();
    // Strict hex: `from_str_radix` alone would also take a sign (`+f`).
    if text.len() != 64 || !text.bytes().all(|byte| byte.is_ascii_hexdigit()) {
      return Err(invalid());
    }
    let mut bytes = (0..32)
      .map(|index| u8::from_str_radix(&text[index * 2..index * 2 + 2], 16))
      .collect::<Result<Vec<_>, _>>()
      .map_err(|_| invalid())?;
    let key = UnboundKey::new(&AES_256_GCM, &bytes).map_err(|_| invalid());
    bytes.zeroize();
    Ok(Self(LessSafeKey::new(key?)))
  }

  /// A fresh random nonce, then the ciphertext and tag; the kid is bound as associated data.
  fn seal(&self, kid: &str, private: &PrivateKey) -> Result<Vec<u8>, CloudError> {
    let nonce = random_bytes::<NONCE_LEN>()?;
    let mut sealed = private.pkcs8().to_vec();
    self
      .0
      .seal_in_place_append_tag(
        Nonce::assume_unique_for_key(nonce),
        Aad::from(kid.as_bytes()),
        &mut sealed,
      )
      .map_err(|_| CloudError::KeyEncryption)?;
    let mut stored = nonce.to_vec();
    stored.append(&mut sealed);
    Ok(stored)
  }

  fn open(&self, kid: &str, stored: &[u8]) -> Result<PrivateKey, CloudError> {
    if stored.len() <= NONCE_LEN {
      return Err(CloudError::KeyEncryption);
    }
    let (nonce, sealed) = stored.split_at(NONCE_LEN);
    let nonce = Nonce::try_assume_unique_for_key(nonce).map_err(|_| CloudError::KeyEncryption)?;
    let mut buffer = sealed.to_vec();
    let opened = self
      .0
      .open_in_place(nonce, Aad::from(kid.as_bytes()), &mut buffer)
      .map_err(|_| CloudError::KeyEncryption)?;
    let private = PrivateKey::from_pkcs8(opened.to_vec());
    // The `PrivateKey` wipes its own copy; the scratch buffer is wiped here.
    buffer.zeroize();
    Ok(private)
  }
}

impl fmt::Debug for KeySealing {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str("KeySealing(<redacted>)")
  }
}
