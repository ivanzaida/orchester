use serde::{Deserialize, Serialize};

use super::{Checkout, IndexEntry, LocalConflict, LocalPath};
use crate::blob::ContentHash;

/// A result write is fingerprinted before applying it. Raw result bytes live in retained content.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolutionWrite {
  pub path: LocalPath,
  pub before: Option<ContentHash>,
  pub working: Option<ContentHash>,
  pub entry: Option<IndexEntry>,
}

/// An interrupted resolution may complete only its recorded files and preserve every other entry.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingResolution {
  pub before: Checkout,
  pub conflict: LocalConflict,
  pub untouched_index: ContentHash,
  pub writes: Vec<ResolutionWrite>,
}
