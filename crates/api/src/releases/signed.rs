//! What CI signs for each release file (`docs/cloud-api.md`, "The signed message"): an Ed25519 signature over a
//! canonical message that names the file's version, platform, architecture, name, size and SHA-256. The app checks it
//! with the release keys built into it before it runs an update; the cloud checks it with the same keys when CI
//! finalizes an upload.

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error};
use sha2::{Digest, Sha256};

use super::{ReleaseArch, ReleaseFileName, ReleaseFormatError, ReleasePlatform, ReleaseVersion, hex};

/// The first line of every signed message: what it is, and the format's version.
pub const SIGNED_MESSAGE_HEADER: &str = "orchester-release-file-v1";
/// Keeps a release key's id apart from every other hash of the same key.
const KEY_ID_LABEL: &[u8] = b"orchester-release-key\0";

/// The SHA-256 of a file's bytes, written as 64 lowercase hex characters.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct FileDigest([u8; 32]);

impl FileDigest {
  pub fn of(bytes: &[u8]) -> Self {
    Self(Sha256::digest(bytes).into())
  }

  pub const fn from_bytes(bytes: [u8; 32]) -> Self {
    Self(bytes)
  }

  pub fn as_bytes(&self) -> &[u8; 32] {
    &self.0
  }

  pub fn to_hex(&self) -> String {
    hex::encode(&self.0)
  }

  pub fn parse(text: &str) -> Result<Self, ReleaseFormatError> {
    hex::decode(text).map(Self).ok_or(ReleaseFormatError::Digest)
  }
}

impl fmt::Debug for FileDigest {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "FileDigest({})", self.to_hex())
  }
}

/// A release key's id: the first 8 bytes of SHA-256(`"orchester-release-key\0"` ‖ the key's 32 bytes), as 16
/// lowercase hex characters. Each signature names the key it was made with, so the app can accept a short list of keys
/// while one replaces another.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct ReleaseKeyId([u8; 8]);

impl ReleaseKeyId {
  /// The id of the Ed25519 public key `key`.
  pub fn of(key: &[u8; 32]) -> Self {
    let mut hash = Sha256::new();
    hash.update(KEY_ID_LABEL);
    hash.update(key);
    let hash: [u8; 32] = hash.finalize().into();
    let mut id = [0_u8; 8];
    id.copy_from_slice(&hash[..8]);
    Self(id)
  }

  pub fn to_hex(&self) -> String {
    hex::encode(&self.0)
  }

  pub fn parse(text: &str) -> Result<Self, ReleaseFormatError> {
    hex::decode(text).map(Self).ok_or(ReleaseFormatError::KeyId)
  }
}

impl fmt::Debug for ReleaseKeyId {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "ReleaseKeyId({})", self.to_hex())
  }
}

impl fmt::Display for ReleaseKeyId {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(&self.to_hex())
  }
}

/// An Ed25519 signature over a [`SignedFile`]'s message, written as 128 lowercase hex characters.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct FileSignature([u8; 64]);

impl FileSignature {
  pub const fn from_bytes(bytes: [u8; 64]) -> Self {
    Self(bytes)
  }

  pub fn as_bytes(&self) -> &[u8; 64] {
    &self.0
  }

  pub fn to_hex(&self) -> String {
    hex::encode(&self.0)
  }

  pub fn parse(text: &str) -> Result<Self, ReleaseFormatError> {
    hex::decode(text).map(Self).ok_or(ReleaseFormatError::Signature)
  }
}

impl fmt::Debug for FileSignature {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "FileSignature({}…)", &self.to_hex()[..16])
  }
}

/// What a signature over a release file vouches for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SignedFile {
  pub version: ReleaseVersion,
  pub platform: ReleasePlatform,
  pub arch: ReleaseArch,
  pub name: ReleaseFileName,
  pub size: u64,
  pub sha256: FileDigest,
}

impl SignedFile {
  /// The exact bytes CI signs: seven lines of UTF-8 (all ASCII, since every field is), each ended by one `\n` (also the
  /// last), no other whitespace, the size in decimal without leading zeros:
  ///
  /// ```text
  /// orchester-release-file-v1
  /// version=<version>
  /// platform=<windows|macos>
  /// arch=<x86_64|aarch64>
  /// name=<file name>
  /// size=<bytes>
  /// sha256=<64 lowercase hex>
  /// ```
  pub fn message(&self) -> Vec<u8> {
    format!(
      "{SIGNED_MESSAGE_HEADER}\nversion={}\nplatform={}\narch={}\nname={}\nsize={}\nsha256={}\n",
      self.version,
      self.platform.as_str(),
      self.arch.as_str(),
      self.name,
      self.size,
      self.sha256.to_hex(),
    )
    .into_bytes()
  }
}

macro_rules! hex_serde {
  ($name:ident, $expecting:literal) => {
    impl Serialize for $name {
      fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_hex())
      }
    }

    impl<'de> Deserialize<'de> for $name {
      fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        Self::parse(&text).map_err(|_| D::Error::custom($expecting))
      }
    }
  };
}

hex_serde!(FileDigest, "a SHA-256 is 64 lowercase hex characters");
hex_serde!(ReleaseKeyId, "a release key id is 16 lowercase hex characters");
hex_serde!(FileSignature, "a signature is 128 lowercase hex characters");
