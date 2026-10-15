//! A digest of what a read found, to tell a read again that found the same without comparing it whole: its size
//! with a SHA-256 of its parts, each led by its length so that parts cannot run into each other.

use sha2::{Digest as _, Sha256};

/// The size of what was read, and a SHA-256 of it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Digest {
  size: u64,
  hash: [u8; 32],
}

impl Digest {
  /// The digest of `parts`, whose size (bytes, or entries) is `size`.
  pub fn of<'a>(size: u64, parts: impl IntoIterator<Item = &'a [u8]>) -> Self {
    let mut hasher = Sha256::new();
    for part in parts {
      hasher.update(u64::try_from(part.len()).unwrap_or(u64::MAX).to_le_bytes());
      hasher.update(part);
    }
    let mut hash = [0; 32];
    hash.copy_from_slice(&hasher.finalize());
    Self { size, hash }
  }
}
