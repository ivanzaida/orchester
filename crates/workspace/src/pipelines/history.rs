//! A pipeline's published versions as its version history shows them: newest first, each with
//! what changed from the version before it and the runs that pinned it. Reading blocks on
//! SQLite; the UI reads it off the UI thread.

use std::{collections::BTreeMap, path::Path, sync::Arc};

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

/// What still holds a version of a pipeline, as the list of versions says it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Use {
  /// The current version: the tasks that use the pipeline.
  Tasks(usize),
  /// An older version: the runs that pinned it and so started on it.
  Runs(usize),
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
    let runs = db.runs_by_pipeline_version(pipeline).map_err(run_store_error)?;
    let versions = db
      .versions::<Pipeline>(pipeline)?
      .into_iter()
      .map(|version| (version.number(), version.published_at(), version.body().clone()));
    Ok(Self::of(versions, runs))
  }

  /// The history of `versions`, oldest first, each with the runs of `runs` that pinned it.
  pub(crate) fn of(
    versions: impl IntoIterator<Item = (VersionNumber, Timestamp, Pipeline)>,
    mut runs: BTreeMap<VersionNumber, Vec<RunNumber>>,
  ) -> Self {
    let mut previous: Option<(VersionNumber, Arc<Pipeline>)> = None;
    let mut list = Vec::new();
    for (number, published_at, body) in versions {
      let body = Arc::new(body);
      list.push(PublishedVersion {
        number,
        published_at,
        changes: previous.as_ref().map(|(before_number, before)| ChangesFrom {
          version: *before_number,
          changes: changes(before, &body),
        }),
        runs: runs.remove(&number).unwrap_or_default(),
        body: body.clone(),
      });
      previous = Some((number, body));
    }
    list.reverse();
    Self { versions: list }
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

  /// What holds version `number`: the tasks that use the pipeline while it is the current (latest) version, the runs
  /// that pinned it otherwise; `None` for a version that was never published.
  pub fn use_of(&self, number: VersionNumber, tasks: usize) -> Option<Use> {
    let version = self.version(number)?;
    Some(if self.latest().is_some_and(|latest| latest.number == number) {
      Use::Tasks(tasks)
    } else {
      Use::Runs(version.runs.len())
    })
  }

  pub fn latest(&self) -> Option<&PublishedVersion> {
    self.versions.first()
  }
}
