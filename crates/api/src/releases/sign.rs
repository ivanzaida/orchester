//! Signing release files with a release key (`docs/cloud-api.md`, "The signed message"): what CI's release tool does
//! with the private key from its secret store, which never leaves that process.

use std::fmt;

use orchester_core::secret::SecretValue;
use ring::{
  rand::{SecureRandom, SystemRandom},
  signature::{Ed25519KeyPair, KeyPair},
};
use zeroize::Zeroize;

use super::{FileSignature, ReleaseKeyId, ReleasePublicKey, SignedFile, hex};

/// Why a release signing key could not be read or made.
#[derive(Clone, Copy, Debug, thiserror::Error, PartialEq, Eq)]
pub enum SigningKeyError {
  #[error("a release signing key is its 32-byte Ed25519 seed as 64 lowercase hex characters")]
  Format,
  #[error("the operating system's random source failed")]
  Random,
}

/// An Ed25519 release key's private half, from its 32-byte seed. `Debug` names only its key id; there is no
/// `Display`, `Clone` or serialization, so the seed reaches no log or copy.
pub struct ReleaseSigningKey {
  pair: Ed25519KeyPair,
  public: ReleasePublicKey,
}

impl ReleaseSigningKey {
  /// The key whose seed is `seed`, 64 lowercase hex characters. The decoded seed is wiped once the key is made.
  pub fn parse(seed: &str) -> Result<Self, SigningKeyError> {
    let mut bytes: [u8; 32] = hex::decode(seed).ok_or(SigningKeyError::Format)?;
    let key = Self::from_seed(&bytes);
    bytes.zeroize();
    key
  }

  /// A new key from the operating system's random source, with its seed as 64 lowercase hex characters: the value
  /// to keep in CI's secret store, shown once.
  pub fn generate() -> Result<(Self, SecretValue), SigningKeyError> {
    let mut bytes = [0_u8; 32];
    SystemRandom::new()
      .fill(&mut bytes)
      .map_err(|_| SigningKeyError::Random)?;
    let key = Self::from_seed(&bytes);
    let seed = SecretValue::new(hex::encode(&bytes)).expect("64 hex characters are never blank");
    bytes.zeroize();
    Ok((key?, seed))
  }

  fn from_seed(seed: &[u8; 32]) -> Result<Self, SigningKeyError> {
    let pair = Ed25519KeyPair::from_seed_unchecked(seed).map_err(|_| SigningKeyError::Format)?;
    let public: [u8; 32] = pair
      .public_key()
      .as_ref()
      .try_into()
      .map_err(|_| SigningKeyError::Format)?;
    Ok(Self {
      pair,
      public: ReleasePublicKey::from_bytes(public),
    })
  }

  /// The public half, which the app and the cloud accept signatures with.
  pub fn public_key(&self) -> ReleasePublicKey {
    self.public
  }

  pub fn id(&self) -> ReleaseKeyId {
    self.public.id()
  }

  /// The signature over `file`'s message ([`SignedFile::message`]).
  pub fn sign(&self, file: &SignedFile) -> FileSignature {
    let signature: [u8; 64] = self
      .pair
      .sign(&file.message())
      .as_ref()
      .try_into()
      .expect("an Ed25519 signature is 64 bytes");
    FileSignature::from_bytes(signature)
  }
}

impl fmt::Debug for ReleaseSigningKey {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "ReleaseSigningKey({})", self.id())
  }
}
