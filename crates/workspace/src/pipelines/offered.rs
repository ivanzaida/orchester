//! Which pipelines a project offers (`docs/architecture/server.md` row 80, the person's decision of 2026-11-05): a
//! local project every one; a team project only the team's, which its server shares, and an Admin's new pipeline
//! until its first publish creates it on the server. A person's own local pipelines of a team project stay in its
//! database as they are and are offered nowhere: not listed, not chosen for a task, not run.

use orchester_core::id::ResourceId;
use orchester_store::{ProjectDb, ResourceSharing, StoreError};

/// The pipelines one project offers.
#[derive(Clone, Copy, Debug)]
pub struct Offered {
  team_project: bool,
}

impl Offered {
  pub fn of(db: &ProjectDb) -> Result<Self, StoreError> {
    Ok(Self {
      team_project: db.sharing()?.is_some(),
    })
  }

  /// Whether the project offers `pipeline`, `published` when it has a version here.
  pub fn offers(self, db: &ProjectDb, pipeline: ResourceId, published: bool) -> Result<bool, StoreError> {
    // A pipeline never published in a team project is an Admin's draft: its first publish makes it the team's.
    if !self.team_project || !published {
      return Ok(true);
    }
    Ok(matches!(
      db.resource_sharing(pipeline)?,
      Some(ResourceSharing::Shared(_))
    ))
  }
}
