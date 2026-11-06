//! A run as the attempt page and the review list name it ([`MergeRun`]), and what it changes ([`run_size`]).

use std::{path::Path, sync::Arc};

use orchester_core::{
  diff::Changes,
  id::{RunId, TaskId},
  project::TaskKey,
  run::{BranchName, Run, RunNumber},
};
use orchester_exec::programs::Programs;

use crate::tasks::changes::{Checkpointed, RunRange, run_changes_in};

/// A run as the attempt page names it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MergeRun {
  pub task: TaskId,
  pub key: TaskKey,
  pub title: Arc<str>,
  pub run: RunId,
  pub number: RunNumber,
  /// The branch the run merges into; `None` when the run did not record it.
  pub base: Option<BranchName>,
}

/// What `run` changes up to its latest checkpoint; `None` when it has none or cannot be read (the log says why).
pub(crate) fn run_size((root, programs): (&Path, &Programs), run: &Run) -> Option<Changes> {
  match run_changes_in(root, programs, &RunRange::of(run)) {
    Ok(Checkpointed::Read { changes, .. }) => Some(Changes {
      files: u32::try_from(changes.diff.files().len()).unwrap_or(u32::MAX),
      additions: changes.diff.additions(),
      deletions: changes.diff.deletions(),
    }),
    Ok(Checkpointed::TooLarge { size, .. }) => Some(size),
    Ok(Checkpointed::NoCheckpoint) => None,
    Err(error) => {
      tracing::warn!(%error, run = %run.id(), "cannot read what a run changes");
      None
    }
  }
}
