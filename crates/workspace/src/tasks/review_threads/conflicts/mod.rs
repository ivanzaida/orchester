//! A merge that found conflicts on the review page (module Review v2; design `Review - v2`, 11 `CT6Gc`): the person
//! approved, the merge into the run's base branch found conflicts and merged nothing, and the run waits for review
//! again. [`standing`] tells whether the run's last merge attempt is such a merge and still stands, [`read`] adds where
//! the conflicts are, and [`resolve`] hands the run back to Implement with the base merged in. *Try merge again* is
//! the approval's own merge ([`super::approve`]), tried once the base moved. Blocks on SQLite and git.

mod regions;
pub mod resolve;

use std::path::Path;

use orchester_core::{
  integration::{attempt::AttemptOutcome, branch_head},
  run::{BranchName, Run},
  time::Timestamp,
};
use orchester_exec::programs::Programs;
use orchester_store::{ProjectDb, StoreError};

pub use self::regions::{BaseLine, ConflictRegion};
use crate::tasks::changes::{Checkpointed, GIT};

/// A merge into the run's base branch that found conflicts and merged nothing, as the review page shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReviewConflicts {
  /// The branch the merge was into; `None` when the run recorded none.
  pub base: Option<BranchName>,
  /// When the merge was tried: when the person approved.
  pub at: Timestamp,
  /// The files that conflict.
  pub files: Vec<String>,
  /// Where they conflict in the run's diff, with what the base has there now; a file with no region here conflicts in
  /// a way its lines do not show (a file one side deleted).
  pub regions: Vec<ConflictRegion>,
}

impl ReviewConflicts {
  /// Whether `path` is one of the files that conflict.
  pub fn conflicts_in(&self, path: &str) -> bool {
    self.files.iter().any(|file| file == path)
  }

  /// The conflicts in `path`'s lines, in the order they stand.
  pub fn regions_in<'a>(&'a self, path: &'a str) -> impl Iterator<Item = &'a ConflictRegion> {
    self.regions.iter().filter(move |region| region.path == path)
  }
}

/// The last merge attempt of a run that found conflicts, while it still stands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Standing {
  pub at: Timestamp,
  pub files: Vec<String>,
}

/// Whether the merge attempt of `run` made at `at` is older than the run's work: Implement committed again after it, so
/// what it found no longer holds, and the run is reviewed afresh.
pub fn is_stale(run: &Run, at: Timestamp) -> bool {
  run
    .last_checkpoint()
    .is_some_and(|checkpoint| checkpoint.created_at() > at)
}

/// The newest merge attempt of `run` that found conflicts, while it stands: the run's work has not changed since, and
/// no attempt landed. Attempts after it that merged nothing for another reason (the merge was refused, Git stopped)
/// leave the conflicts as they were.
pub fn standing(db: &ProjectDb, run: &Run) -> Result<Option<Standing>, StoreError> {
  let attempts = db.merge_attempts(run.id())?;
  if attempts.iter().any(|recorded| recorded.attempt.outcome().landed()) {
    return Ok(None);
  }
  Ok(attempts.into_iter().find_map(|recorded| {
    let attempt = recorded.attempt;
    match attempt.outcome() {
      AttemptOutcome::Conflicts { files } if !is_stale(run, attempt.at()) => Some(Standing {
        at: attempt.at(),
        files: files.clone(),
      }),
      _ => None,
    }
  }))
}

/// The conflicts of `run` for its review page, where `changes` is the run's diff: where they stand in it and what the
/// base has there now. A region git cannot be asked about is left out and logged; the files stay.
pub fn read(
  (root, programs): (&Path, &Programs),
  run: &Run,
  (standing, changes): (Standing, &Checkpointed),
) -> ReviewConflicts {
  let regions = match (changes, run.base_branch()) {
    (Checkpointed::Read { changes, .. }, Some(base)) => {
      let tip = branch_head(&programs.runner(), Path::new(GIT), (root, base)).unwrap_or_else(|error| {
        tracing::warn!(%error, run = %run.id(), "cannot read where the run's base branch is");
        None
      });
      tip.map_or_else(Vec::new, |tip| {
        regions::read(
          (root, programs),
          (&changes.from, &tip),
          (&changes.diff, &standing.files),
        )
      })
    }
    _ => Vec::new(),
  };
  ReviewConflicts {
    base: run.base_branch().cloned(),
    at: standing.at,
    files: standing.files,
    regions,
  }
}
