//! *Record merge* on a run's attempt page (`crates/desktop-app/design/orchester.pen`, board `Review · redesign`, 09g
//! `i2rVv`): Git merged the run, but Orchester could not record it (`AttemptOutcome::NotRecorded`). The merge Git made
//! is recorded as it is, once the base branch still holds it, and what the run left is cleared as after any merge;
//! nothing is merged again. Holds what a merge holds meanwhile. Blocks on SQLite and git.

use std::path::Path;

use orchester_core::{
  id::RunId,
  integration::{MergeError, RunMerge, attempt::AttemptOutcome, merge_made},
  store::{MergeStore, RunStore},
};
use orchester_exec::programs::Programs;
use orchester_store::ProjectDb;

use super::{
  changes::GIT,
  detail::run_store_error,
  integration::{IntegrationProblem, clear, hold},
};
use crate::messages::Message;

/// Records the merge Git made of `run` in the project at `root`, as its last attempt names it; the merge recorded. A
/// run merged already, one whose last attempt was not merged by Git, and a merge its base branch no longer holds are
/// refused.
pub fn record_merge(root: &Path, programs: &Programs, run: RunId) -> Result<RunMerge, IntegrationProblem> {
  let _held = hold(root)?;
  let mut db = ProjectDb::open(root)?;
  if db.run_merge(run)?.is_some() {
    return Err(IntegrationProblem::AlreadyMerged);
  }
  let last = db.merge_attempts(run)?.into_iter().next();
  let Some((AttemptOutcome::NotRecorded { revision }, at)) =
    last.map(|recorded| (recorded.attempt.outcome().clone(), recorded.attempt.at()))
  else {
    return Err(IntegrationProblem::Store(Message::new(
      "runs.merge.failure.nothingToRecord",
    )));
  };
  let stored = db
    .run(run)
    .map_err(run_store_error)?
    .ok_or(IntegrationProblem::Store(Message::new("errors.storage")))?;
  let into = stored
    .base_branch()
    .cloned()
    .ok_or(IntegrationProblem::Merge(MergeError::NoBaseBranch))?;
  let kind = merge_made(
    &programs.runner(),
    Path::new(GIT),
    (root, &into, stored.branch()),
    &revision,
  )
  .map_err(IntegrationProblem::Read)?
  .ok_or_else(|| {
    IntegrationProblem::Store(Message::new("runs.merge.failure.mergeGone").with("branch", into.to_string()))
  })?;
  let merge = RunMerge {
    run,
    into,
    revision,
    kind,
    merged_at: at,
  };
  db.save_run_merge(&merge)?;
  super::local_integration::settle_review(&mut db, &merge);
  crate::base_moved::after_merge(root, programs);
  if let Err(problem) = clear(&mut db, programs, root, &stored, None, false) {
    tracing::warn!(?problem, %run, "the merge was recorded, but what the run left could not be cleared");
  }
  Ok(merge)
}
