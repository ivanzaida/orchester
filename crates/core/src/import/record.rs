//! What the target keeps of an import (the result `z3vhzo`): the copies it created and where each came from,
//! and what it used of the target's or left unresolved. For display only: nothing follows the source.

use super::batch::{Provenance, Resolved};
use crate::{id::ResourceId, time::Timestamp};

/// The import that created a resource, as the target recorded it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImportRecord {
  pub imported_at: Timestamp,
  /// Every copy the import created, in the order it created them, with where it came from.
  pub copies: Vec<(ResourceId, Provenance)>,
  /// What it used of the target's instead of copying it, and what it left unresolved, in plan order.
  pub resolved: Vec<Resolved>,
}

impl ImportRecord {
  /// Where the copy `resource` came from, if the import created it.
  pub fn provenance(&self, resource: ResourceId) -> Option<&Provenance> {
    self
      .copies
      .iter()
      .find(|(copy, _)| *copy == resource)
      .map(|(_, provenance)| provenance)
  }
}
