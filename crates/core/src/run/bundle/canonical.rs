//! The canonical bytes of an execution bundle and its hash, the rule server and desktop share so that the same bundle
//! always hashes the same:
//!
//! 1. The bytes are the value's compact JSON in UTF-8 as `serde_json` writes it: no whitespace between tokens, strings
//!    escaped as `serde_json` escapes them, integers in decimal.
//! 2. An object's fields come in the order its type declares them, which is the order its documented wire shape lists
//!    them; a map's entries come in byte order of their keys (`BTreeMap`). No type a bundle holds is a `HashMap` or a
//!    `serde_json::Value`, whose order is not fixed.
//! 3. A collection whose order means nothing (the pinned profiles, instructions, tools and skills, the files) is sorted
//!    by its constructor and when it is read, by resource id or by hash; one whose order means something (the
//!    pipeline's steps, a step's instructions, a body's lists as published) keeps it.
//! 4. Optional fields a type leaves out while empty stay out; those it sends as `null` stay `null`.
//! 5. The hash is the lowercase hex SHA-256 of those bytes.
//!
//! Only a value built or read through its constructors is canonical: hash the value, never bytes someone sent.

use serde::Serialize;
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::blob::ContentHash;

/// A value that cannot be written as JSON, e.g. a map whose keys are not strings. No bundle type is one.
#[derive(Debug, Error)]
#[error("the value has no canonical JSON form: {0}")]
pub struct CanonicalError(#[from] serde_json::Error);

/// `value`'s canonical bytes, by the rule of this module.
pub fn canonical_bytes<T: Serialize>(value: &T) -> Result<Vec<u8>, CanonicalError> {
  Ok(serde_json::to_vec(value)?)
}

/// The SHA-256 of `value`'s canonical bytes: an execution bundle's hash.
pub fn bundle_sha256<T: Serialize>(value: &T) -> Result<ContentHash, CanonicalError> {
  canonical_bytes(value).map(|bytes| file_sha256(&bytes))
}

/// The SHA-256 of a file's bytes, as a bundle's file manifests name it.
pub fn file_sha256(bytes: &[u8]) -> ContentHash {
  let hex: String = Sha256::digest(bytes).iter().map(|byte| format!("{byte:02x}")).collect();
  ContentHash::new(&hex).expect("a SHA-256 in lowercase hex is a content hash")
}
