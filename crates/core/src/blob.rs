//! Content addresses of project blobs (`docs/architecture/storage.md`, "Blobs"): large or binary
//! content a resource refers to instead of holding it, e.g. a skill's reference files.

use std::fmt;

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ContentHashError {
  #[error("a content hash is 64 lowercase hexadecimal digits, got `{0}`")]
  Invalid(String),
}

/// The SHA-256 of a blob's content, in lowercase hex. The store computes it when it stores the
/// content ([`crate::store::BlobStore::put_blob`]); anything else only passes it on.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ContentHash(String);

impl ContentHash {
  pub fn new(value: &str) -> Result<Self, ContentHashError> {
    let valid = value.len() == 64 && value.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
    if valid {
      Ok(Self(value.to_owned()))
    } else {
      Err(ContentHashError::Invalid(value.to_owned()))
    }
  }

  pub fn as_str(&self) -> &str {
    &self.0
  }
}

impl fmt::Display for ContentHash {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(&self.0)
  }
}

impl TryFrom<String> for ContentHash {
  type Error = ContentHashError;

  fn try_from(value: String) -> Result<Self, ContentHashError> {
    Self::new(&value)
  }
}

impl From<ContentHash> for String {
  fn from(hash: ContentHash) -> Self {
    hash.0
  }
}
