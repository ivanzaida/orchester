//! A team server's certificate as people compare it and the desktop pins it (`docs/architecture/server.md`,
//! "Transport and certificates"): the SHA-256 of the leaf certificate, and its check code.

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error};
use sha2::{Digest, Sha256};

/// Crockford's base32 alphabet: no I, L, O or U, so a code read aloud or typed is not misread.
pub const CROCKFORD: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

/// `bytes` in Crockford base32, five bits a character; `bytes.len()` is a multiple of five.
pub fn crockford(bytes: &[u8]) -> String {
  debug_assert!(bytes.len().is_multiple_of(5), "whole 40-bit groups only");
  let mut text = String::with_capacity(bytes.len() / 5 * 8);
  for group in bytes.chunks(5) {
    let bits = group.iter().fold(0_u64, |bits, byte| bits << 8 | u64::from(*byte));
    for shift in (0..8).rev() {
      text.push(char::from(CROCKFORD[((bits >> (shift * 5)) & 31) as usize]));
    }
  }
  text
}

/// The SHA-256 of a certificate's DER bytes. The desktop accepts exactly the certificate it pinned, whatever names or
/// dates it carries. Serialized as 64 lowercase hex characters.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct CertificatePin([u8; 32]);

impl CertificatePin {
  pub fn of(certificate_der: &[u8]) -> Self {
    Self(Sha256::digest(certificate_der).into())
  }

  pub fn as_bytes(&self) -> &[u8; 32] {
    &self.0
  }

  /// The check code people compare: the first 80 bits as 16 Crockford characters in four groups,
  /// `3F7A-92C1-KD8M-QX4T`.
  pub fn check_code(&self) -> String {
    let text = crockford(&self.0[..10]);
    let groups: Vec<&str> = (0..4).map(|group| &text[group * 4..group * 4 + 4]).collect();
    groups.join("-")
  }

  fn hex(&self) -> String {
    self.0.iter().map(|byte| format!("{byte:02x}")).collect()
  }

  pub fn parse(text: &str) -> Option<Self> {
    if text.len() != 64 || !text.bytes().all(|byte| byte.is_ascii_hexdigit()) {
      return None;
    }
    let mut bytes = [0_u8; 32];
    for (index, byte) in bytes.iter_mut().enumerate() {
      *byte = u8::from_str_radix(&text[index * 2..index * 2 + 2], 16).ok()?;
    }
    Some(Self(bytes))
  }
}

impl fmt::Debug for CertificatePin {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "CertificatePin({})", self.check_code())
  }
}

impl Serialize for CertificatePin {
  fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(&self.hex())
  }
}

impl<'de> Deserialize<'de> for CertificatePin {
  fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
    let text = String::deserialize(deserializer)?;
    Self::parse(&text).ok_or_else(|| D::Error::custom("a certificate pin is 64 hex characters"))
  }
}
