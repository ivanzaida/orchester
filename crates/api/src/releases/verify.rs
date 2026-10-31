//! Checking a release file's signature with the release keys an app or the cloud accepts (`docs/cloud-api.md`, "Key
//! rotation"): the signature names its key by id, and only a key on the list verifies it.

use ring::signature::{ED25519, UnparsedPublicKey};

use super::{FileSignature, ReleaseFormatError, ReleaseKeyId, SignedFile, hex};

/// Why a release file's signature was refused.
#[derive(Clone, Copy, Debug, thiserror::Error, PartialEq, Eq)]
pub enum SignatureError {
  /// The signature names a key that is not on the accepted list (one retired, or never issued).
  #[error("the signature's key is not accepted")]
  UnknownKey,
  /// The named key does not verify the signature over this file's message.
  #[error("the signature does not verify")]
  BadSignature,
}

/// An Ed25519 public key that signs release files, written as 64 lowercase hex characters.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReleasePublicKey {
  id: ReleaseKeyId,
  bytes: [u8; 32],
}

impl ReleasePublicKey {
  pub fn from_bytes(bytes: [u8; 32]) -> Self {
    Self {
      id: ReleaseKeyId::of(&bytes),
      bytes,
    }
  }

  pub fn parse(text: &str) -> Result<Self, ReleaseFormatError> {
    hex::decode(text)
      .map(Self::from_bytes)
      .ok_or(ReleaseFormatError::PublicKey)
  }

  pub fn id(&self) -> ReleaseKeyId {
    self.id
  }

  pub fn as_bytes(&self) -> &[u8; 32] {
    &self.bytes
  }

  pub fn to_hex(&self) -> String {
    hex::encode(&self.bytes)
  }
}

/// The release keys an app has built in, or the cloud is configured with: the current key and, while one replaces
/// another, the one before it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AcceptedReleaseKeys(Vec<ReleasePublicKey>);

impl AcceptedReleaseKeys {
  pub fn new(keys: Vec<ReleasePublicKey>) -> Self {
    Self(keys)
  }

  /// Whether `signature`, made with the key `key_id`, is a signature over `file`'s message by a key on the list.
  pub fn verify(
    &self,
    file: &SignedFile,
    key_id: ReleaseKeyId,
    signature: &FileSignature,
  ) -> Result<(), SignatureError> {
    let key = self
      .0
      .iter()
      .find(|key| key.id == key_id)
      .ok_or(SignatureError::UnknownKey)?;
    UnparsedPublicKey::new(&ED25519, &key.bytes)
      .verify(&file.message(), signature.as_bytes())
      .map_err(|_| SignatureError::BadSignature)
  }
}
