//! A pipeline's published versions as its version history shows them: newest first, each with
//! what changed from the version before it and the runs that pinned it. Reading blocks on
//! SQLite; the UI reads it off the UI thread.

use std::{path::Path, sync::Arc};

use orchester_core::{
  id::ResourceId,
  resource::{Pipeline, PipelineChange, VersionNumber, changes},
  run::RunNumber,
  store::{ResourceStore, RunStore},
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};

use crate::tasks::detail::run_store_error;

/// One published version.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PublishedVersion {
  pub number: VersionNumber,
  pub published_at: Timestamp,
  pub body: Arc<Pipeline>,
  /// What changed from the version before; `None` for the first version.
  pub changes: Option<ChangesFrom>,
  /// Runs that pinned this version, in run order.
  pub runs: Vec<RunNumber>,
}

/// What changed from an earlier version.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChangesFrom {
  pub version: VersionNumber,
  pub changes: Vec<PipelineChange>,
}

/// Every published version of a pipeline, newest first.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct VersionHistory {
  versions: Vec<PublishedVersion>,
}

impl VersionHistory {
  pub fn load(db: &ProjectDb, pipeline: ResourceId) -> Result<Self, StoreError> {
    let mut runs = db.runs_by_pipeline_version(pipeline).map_err(run_store_error)?;
    let mut previous: Option<(VersionNumber, Arc<Pipeline>)> = None;
    let mut versions = Vec::new();
    for version in db.versions::<Pipeline>(pipeline)? {
      let body = Arc::new(version.body().clone());
      versions.push(PublishedVersion {
        number: version.number(),
        published_at: version.published_at(),
        changes: previous.as_ref().map(|(number, before)| ChangesFrom {
          version: *number,
          changes: changes(before, &body),
        }),
        runs: runs.remove(&version.number()).unwrap_or_default(),
        body: body.clone(),
      });
      previous = Some((version.number(), body));
    }
    versions.reverse();
    Ok(Self { versions })
  }

  /// [`Self::load`] from the project database of `repo_root`.
  pub fn load_in(repo_root: &Path, pipeline: ResourceId) -> Result<Self, StoreError> {
    Self::load(&ProjectDb::open(repo_root)?, pipeline)
  }

  /// Newest first.
  pub fn versions(&self) -> &[PublishedVersion] {
    &self.versions
  }

  pub fn version(&self, number: VersionNumber) -> Option<&PublishedVersion> {
    self.versions.iter().find(|version| version.number == number)
  }

  pub fn latest(&self) -> Option<&PublishedVersion> {
    self.versions.first()
  }
}
