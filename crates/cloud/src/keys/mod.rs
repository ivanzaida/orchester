//! The cloud's signing keys and key sets (`docs/architecture/server.md`, "Identity tokens and keys").
//!
//! A signing key signs identity tokens for 30 days and stays listed 60 more. The cloud's own root key, made at its
//! first start and sealed in the database like the signing keys, signs each key set. The cloud keeps the schedule
//! itself (`schedule`) and signs tokens only with a key the newest published, unexpired key set lists.

mod issue;
mod root;
mod schedule;

use std::fmt;

use orchester_api::{
  Credential,
  identity::{CLOCK_SKEW_SECS, KEY_LISTED_AFTER_SECS, KEY_SIGNS_SECS, PrivateKey},
};
use ring::aead::{AES_256_GCM, Aad, LessSafeKey, NONCE_LEN, Nonce, UnboundKey};
pub(crate) use root::open as open_root;
use zeroize::{Zeroize, Zeroizing};

use crate::{clock::DAY_MS, error::CloudError, tokens::random_bytes};

const SIGNS_MS: i64 = KEY_SIGNS_SECS * 1000;
const LISTED_AFTER_MS: i64 = KEY_LISTED_AFTER_SECS * 1000;
/// A key set dated later than this from now (a clock set back, or a row written by hand) is neither served nor taken
/// as the newest: team servers would refuse it as not yet valid.
const CLOCK_SKEW_MS: i64 = CLOCK_SKEW_SECS * 1000;
/// A rotated key waits this long before it signs, so the apps and admin sign-ins that carry key sets to team servers
/// bring it there first (review 1, L4). The first key signs at once: no team server holds a key set before it.
const OVERLAP_MS: i64 = 2 * DAY_MS;
/// A new signing key is made once no key would sign a week from now, which leaves room for the two days' overlap. A
/// cloud that was stopped past that point starts the new key when the current one stops signing, never later, so
/// there is no time without a key that signs (`schedule::rotate_if_due`).
const ROTATE_AHEAD_MS: i64 = 7 * DAY_MS;
/// A new key set is signed at least this often; one lives 30 days (`orchester_api::identity::KEY_SET_LIFETIME_SECS`).
const KEY_SET_EVERY_MS: i64 = 7 * DAY_MS;
/// "orchkeys" in ASCII: the advisory lock under which the cloud makes its root, rotates its signing keys and signs key
/// sets, so replicas that run together do each step once.
const LOCK: i64 = 0x6f72_6368_6b65_7973;

/// The key-encryption key's 32 bytes, from its 64 hex characters; wiped when dropped.
pub(crate) fn key_encryption_key(key: &Credential) -> Result<Zeroizing<Vec<u8>>, CloudError> {
  let text = key.expose().trim();
  // Strict hex: `from_str_radix` alone would also take a sign (`+f`).
  if text.len() != 64 || !text.bytes().all(|byte| byte.is_ascii_hexdigit()) {
    return Err(invalid_key());
  }
  (0..32)
    .map(|index| u8::from_str_radix(&text[index * 2..index * 2 + 2], 16))
    .collect::<Result<Vec<_>, _>>()
    .map(Zeroizing::new)
    .map_err(|_| invalid_key())
}

const fn invalid_key() -> CloudError {
  CloudError::Configuration("ORCHESTER_CLOUD_KEY_ENCRYPTION_KEY must be 64 hex characters")
}

/// Seals the root's and the signing keys' private halves with the key-encryption key from the host's environment.
pub(crate) struct KeySealing(LessSafeKey);

impl KeySealing {
  /// The key-encryption key is 64 hex characters (32 bytes).
  pub fn new(key: &Credential) -> Result<Self, CloudError> {
    let bytes = key_encryption_key(key)?;
    let key = UnboundKey::new(&AES_256_GCM, &bytes).map_err(|_| invalid_key())?;
    Ok(Self(LessSafeKey::new(key)))
  }

  /// A fresh random nonce, then the ciphertext and tag; the key's label (a signing key's kid) is bound as associated
  /// data.
  fn seal(&self, label: &str, private: &PrivateKey) -> Result<Vec<u8>, CloudError> {
    let nonce = random_bytes::<NONCE_LEN>()?;
    let mut sealed = private.pkcs8().to_vec();
    self
      .0
      .seal_in_place_append_tag(
        Nonce::assume_unique_for_key(nonce),
        Aad::from(label.as_bytes()),
        &mut sealed,
      )
      .map_err(|_| CloudError::KeyEncryption)?;
    let mut stored = nonce.to_vec();
    stored.append(&mut sealed);
    Ok(stored)
  }

  /// `StoredData` for a blob too short to be sealed at all; `KeyEncryption` when the key-encryption key does not open
  /// it (another key, or a blob changed after sealing).
  fn open(&self, label: &str, stored: &[u8]) -> Result<PrivateKey, CloudError> {
    if stored.len() <= NONCE_LEN + AES_256_GCM.tag_len() {
      return Err(CloudError::StoredData);
    }
    let (nonce, sealed) = stored.split_at(NONCE_LEN);
    let nonce = Nonce::try_assume_unique_for_key(nonce).map_err(|_| CloudError::StoredData)?;
    let mut buffer = sealed.to_vec();
    let opened = self
      .0
      .open_in_place(nonce, Aad::from(label.as_bytes()), &mut buffer)
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
