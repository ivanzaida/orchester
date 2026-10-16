//! A digest of what a read found, to tell a read again that found the same without comparing it whole: what kind
//! of thing it is, its size, and a SHA-256 of its parts, each led by its length so that parts cannot run into each
//! other. The kind keeps a binary file and an empty text of the same size apart.

use sha2::{Digest as _, Sha256};

/// What a digest was made of.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
  Text,
  List,
  Binary,
  TooLarge,
}

/// The kind and size of what was read, and a SHA-256 of it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Digest {
  kind: Kind,
  size: u64,
  hash: [u8; 32],
}

impl Digest {
  /// The digest of a text of `size` bytes, its `lines`.
  pub fn text<'a>(size: u64, lines: impl IntoIterator<Item = &'a [u8]>) -> Self {
    Self::of(Kind::Text, size, lines)
  }

  /// The digest of a list of `count` `paths`.
  pub fn list<'a>(count: u64, paths: impl IntoIterator<Item = &'a [u8]>) -> Self {
    Self::of(Kind::List, count, paths)
  }

  /// The digest of a binary file of `size` bytes, whose content is not shown.
  pub fn binary(size: u64) -> Self {
    Self::of(Kind::Binary, size, [])
  }

  /// The digest of a file of `size` bytes, too large to be shown.
  pub fn too_large(size: u64) -> Self {
    Self::of(Kind::TooLarge, size, [])
  }

  fn of<'a>(kind: Kind, size: u64, parts: impl IntoIterator<Item = &'a [u8]>) -> Self {
    let mut hasher = Sha256::new();
    for part in parts {
      hasher.update(u64::try_from(part.len()).unwrap_or(u64::MAX).to_le_bytes());
      hasher.update(part);
    }
    let mut hash = [0; 32];
    hash.copy_from_slice(&hasher.finalize());
    Self { kind, size, hash }
  }
}
