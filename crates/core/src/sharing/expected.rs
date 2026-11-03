//! The revisions a team-authority command expects: the server commits the command only while every one still holds.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::id::{ModuleId, ProjectId, ResourceId, TagId, TaskId, WorkId};

/// The record an expected revision names; each kind takes only its own id type, so a kind and id cannot mismatch.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RevisionTarget {
  Project(ProjectId),
  Task(TaskId),
  Module(ModuleId),
  Tag(TagId),
  Resource(ResourceId),
  Work(WorkId),
}

#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum ExpectedRevisionError {
  /// Revisions start at 1 and are stored signed, so 0 and anything above `i64::MAX` were never committed.
  #[error("an expected revision is between 1 and i64::MAX, got {0}")]
  OutOfRange(u64),
  #[error("expected revisions name {0:?} more than once")]
  Duplicate(RevisionTarget),
}

/// One record's revision a command was decided against. Wire: `{kind, id, revision}`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "Wire", into = "Wire")]
pub struct ExpectedRevision {
  target: RevisionTarget,
  revision: u64,
}

impl ExpectedRevision {
  pub fn new(target: RevisionTarget, revision: u64) -> Result<Self, ExpectedRevisionError> {
    if revision == 0 || revision > i64::MAX as u64 {
      return Err(ExpectedRevisionError::OutOfRange(revision));
    }
    Ok(Self { target, revision })
  }

  pub const fn target(&self) -> RevisionTarget {
    self.target
  }

  pub const fn revision(&self) -> u64 {
    self.revision
  }
}

/// The expected revisions of one command, at most one per record. May be empty.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "Vec<ExpectedRevision>", into = "Vec<ExpectedRevision>")]
pub struct ExpectedRevisions(Vec<ExpectedRevision>);

impl ExpectedRevisions {
  pub fn new(revisions: Vec<ExpectedRevision>) -> Result<Self, ExpectedRevisionError> {
    let mut seen = HashSet::with_capacity(revisions.len());
    if let Some(duplicate) = revisions.iter().find(|expected| !seen.insert(expected.target)) {
      return Err(ExpectedRevisionError::Duplicate(duplicate.target));
    }
    Ok(Self(revisions))
  }

  pub fn as_slice(&self) -> &[ExpectedRevision] {
    &self.0
  }
}

impl TryFrom<Vec<ExpectedRevision>> for ExpectedRevisions {
  type Error = ExpectedRevisionError;

  fn try_from(revisions: Vec<ExpectedRevision>) -> Result<Self, Self::Error> {
    Self::new(revisions)
  }
}

impl From<ExpectedRevisions> for Vec<ExpectedRevision> {
  fn from(revisions: ExpectedRevisions) -> Self {
    revisions.0
  }
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum Wire {
  Project { id: ProjectId, revision: u64 },
  Task { id: TaskId, revision: u64 },
  Module { id: ModuleId, revision: u64 },
  Tag { id: TagId, revision: u64 },
  Resource { id: ResourceId, revision: u64 },
  Work { id: WorkId, revision: u64 },
}

impl TryFrom<Wire> for ExpectedRevision {
  type Error = ExpectedRevisionError;

  fn try_from(wire: Wire) -> Result<Self, Self::Error> {
    let (target, revision) = match wire {
      Wire::Project { id, revision } => (RevisionTarget::Project(id), revision),
      Wire::Task { id, revision } => (RevisionTarget::Task(id), revision),
      Wire::Module { id, revision } => (RevisionTarget::Module(id), revision),
      Wire::Tag { id, revision } => (RevisionTarget::Tag(id), revision),
      Wire::Resource { id, revision } => (RevisionTarget::Resource(id), revision),
      Wire::Work { id, revision } => (RevisionTarget::Work(id), revision),
    };
    Self::new(target, revision)
  }
}

impl From<ExpectedRevision> for Wire {
  fn from(expected: ExpectedRevision) -> Self {
    let revision = expected.revision;
    match expected.target {
      RevisionTarget::Project(id) => Self::Project { id, revision },
      RevisionTarget::Task(id) => Self::Task { id, revision },
      RevisionTarget::Module(id) => Self::Module { id, revision },
      RevisionTarget::Tag(id) => Self::Tag { id, revision },
      RevisionTarget::Resource(id) => Self::Resource { id, revision },
      RevisionTarget::Work(id) => Self::Work { id, revision },
    }
  }
}
