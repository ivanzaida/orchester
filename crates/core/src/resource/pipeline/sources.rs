//! The sources a pipeline lists (`crate::source`), kept in its versions like its steps.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::{Step, StepKey};
use crate::source::SourcePath;

/// The sources a pipeline lists (`crate::source`): its own, and each step's. A run reads them all.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PipelineSources {
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pipeline: Vec<SourcePath>,
  #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
  steps: BTreeMap<StepKey, Vec<SourcePath>>,
}

/// Why sources do not fit a pipeline.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum PipelineSourcesError {
  #[error("sources are listed for step `{0}`, which is not a step of this pipeline")]
  UnknownStep(StepKey),
}

impl PipelineSources {
  /// The pipeline's own sources, `pipeline`, and the sources of each step, `steps`; a path is kept once per list.
  pub fn new(pipeline: Vec<SourcePath>, steps: BTreeMap<StepKey, Vec<SourcePath>>) -> Self {
    let once = |mut paths: Vec<SourcePath>| {
      let mut seen = BTreeSet::new();
      paths.retain(|path| seen.insert(path.clone()));
      paths
    };
    Self {
      pipeline: once(pipeline),
      steps: steps
        .into_iter()
        .map(|(step, paths)| (step, once(paths)))
        .filter(|(_, paths)| !paths.is_empty())
        .collect(),
    }
  }

  pub fn is_empty(&self) -> bool {
    self.pipeline.is_empty() && self.steps.is_empty()
  }

  /// The pipeline's own sources.
  pub fn pipeline(&self) -> &[SourcePath] {
    &self.pipeline
  }

  /// The sources step `key` lists.
  pub fn step(&self, key: &StepKey) -> &[SourcePath] {
    self.steps.get(key).map_or(&[], Vec::as_slice)
  }

  /// The steps that list sources.
  pub(super) fn step_keys(&self) -> impl Iterator<Item = &StepKey> {
    self.steps.keys()
  }

  /// These sources without those of steps `keep` does not hold, as after a step was removed.
  #[must_use]
  pub fn of_steps(mut self, keep: &[Step]) -> Self {
    self.steps.retain(|key, _| keep.iter().any(|step| &step.key == key));
    self
  }
}
