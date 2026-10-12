use serde::{Deserialize, Serialize};

use super::{Checkout, LocalConflict, Placement};
use crate::blob::ContentHash;

/// Raw editable candidates are private recovery bytes, not a resolved file or an approval.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditorCandidate {
  text: String,
  destination: String,
  placement: Placement,
}

impl EditorCandidate {
  pub fn new(text: String, destination: String, placement: Placement) -> Self {
    Self {
      text,
      destination,
      placement,
    }
  }
  pub fn text(&self) -> &str {
    &self.text
  }
  pub fn destination(&self) -> &str {
    &self.destination
  }
  pub const fn placement(&self) -> Placement {
    self.placement
  }
}

impl std::fmt::Debug for EditorCandidate {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.write_str("EditorCandidate(<private>)")
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConflictKind {
  Task,
  Restoration,
}

/// A durable draft is bound to its source snapshots and still-unresolved conflict.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DraftIdentity {
  pub kind: ConflictKind,
  pub main: Checkout,
  pub task: Option<Checkout>,
  pub file: LocalConflict,
  pub expected: Option<ContentHash>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DraftRecord {
  pub identity: DraftIdentity,
  pub content: ContentHash,
}
