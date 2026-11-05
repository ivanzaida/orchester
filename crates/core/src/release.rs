//! Releases (`docs/design/roadmap.md`): what ships, when, and as which version. A release has a name unique in its
//! project, a description, an optional planned day and is planned or released; a task is in at most one release, as a
//! fix version, whatever module it is in. Orchester records a release; a person ships it and marks it released, which
//! is final.
//!
//! Only the details a person sets and the state are here so far, the shapes a team server's releases are made of
//! (`orchester-api`); the order of releases, the move on *Mark released* and the changelog's composition come with the
//! local releases.

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use crate::name::Name;

/// Whether a release still waits to ship. A released one is final: it is never planned again.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseState {
  Planned,
  Released,
}

/// What a person sets of a release.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReleaseDetails {
  name: Name,
  description: String,
  planned: Option<NaiveDate>,
}

impl ReleaseDetails {
  /// The details, the description trimmed. Any planned day is one: a day already past makes the release overdue, not
  /// invalid.
  pub fn new(name: Name, description: &str, planned: Option<NaiveDate>) -> Self {
    Self {
      name,
      description: description.trim().to_owned(),
      planned,
    }
  }

  pub const fn name(&self) -> &Name {
    &self.name
  }

  /// Markdown, empty when it has none.
  pub fn description(&self) -> &str {
    &self.description
  }

  /// The whole local day it is planned to ship on.
  pub const fn planned(&self) -> Option<NaiveDate> {
    self.planned
  }
}
