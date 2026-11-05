//! Which pipelines a project offers (`docs/architecture/server.md` row 80, the person's decision of 2026-11-05): a
//! local project every one; a team project only the team's: those its server shares, and an Admin's new ones until the
//! server has them (a draft never published, or a version published here that waits for the server). A person's own
//! local pipelines of a team project stay in its database as they are and are offered nowhere: not listed, not chosen
//! for a task, not run. An Admin makes one the team's with the app MCP tool `share_pipeline`.

use std::collections::HashSet;

use orchester_core::id::ResourceId;
use orchester_store::{ProjectDb, ResourceSharing, StoreError};

/// The pipelines one project offers.
#[derive(Clone, Debug)]
pub struct Offered {
  team_project: bool,
  /// The resources with a version published here that waits for the server: the team's, not created there yet.
  waiting: HashSet<ResourceId>,
}

impl Offered {
  pub fn of(db: &ProjectDb) -> Result<Self, StoreError> {
    let team_project = db.sharing()?.is_some();
    let waiting = if team_project {
      db.waiting_publishes()?
        .into_iter()
        .map(|waiting| waiting.resource)
        .collect()
    } else {
      HashSet::new()
    };
    Ok(Self { team_project, waiting })
  }

  /// Whether the project offers `pipeline`, `published` when it has a version here.
  pub fn offers(&self, db: &ProjectDb, pipeline: ResourceId, published: bool) -> Result<bool, StoreError> {
    if !self.team_project || !published || self.waiting.contains(&pipeline) {
      return Ok(true);
    }
    Ok(matches!(
      db.resource_sharing(pipeline)?,
      Some(ResourceSharing::Shared(_))
    ))
  }
}
