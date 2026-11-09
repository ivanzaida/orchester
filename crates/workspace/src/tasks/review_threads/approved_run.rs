//! A run a person approved, as its review page shows it read-only (ORC-800; `Review - v2` 07 `eidig`): merged into its
//! base, or approved in the merge queue, which delivers it in its turn. Who approved it, and when, is what the merge
//! attempts and the queue record; a run merged by automation's policy, or before attempts were recorded, was approved
//! by no one this reads, and says so. Blocks on SQLite.

use orchester_core::{
  id::RunId,
  integration::{MergeKind, RunMerge, attempt::MergeAsker},
  revision::Revision,
  run::BranchName,
  store::MergeStore,
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};

/// Who approved the merge of a run, as far as what Orchester recorded tells.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Approval {
  /// The person approved it at this time: asked for the merge from the page, or approved it in the merge queue.
  Person(Timestamp),
  /// Automation merged it by its policy, or the queue merged it with no approval recorded (an Auto-merge pipeline's
  /// run): no person approved it.
  Automatic,
  /// The merge was recorded without the attempt that says who asked for it.
  NotRecorded,
}

impl Approval {
  /// When a person approved it; `None` when no one is known to have.
  pub const fn person_at(&self) -> Option<Timestamp> {
    match self {
      Self::Person(at) => Some(*at),
      Self::Automatic | Self::NotRecorded => None,
    }
  }
}

/// How the merge of an approved run landed in its base branch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Landed {
  /// The branch it merged into.
  pub into: BranchName,
  /// The commit the base branch was at afterwards.
  pub revision: Revision,
  pub kind: MergeKind,
  pub at: Timestamp,
}

/// An approved run: merged (`landed`), or still waiting for the merge queue's turn (`landed` is `None`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApprovedRun {
  pub approval: Approval,
  pub landed: Option<Landed>,
}

/// Who approved `merge`, which landed.
pub(crate) fn approval_of(db: &ProjectDb, merge: &RunMerge) -> Result<Approval, StoreError> {
  let attempts = db.merge_attempts(merge.run)?;
  let Some(landed) = attempts.iter().find(|recorded| recorded.attempt.outcome().landed()) else {
    return Ok(Approval::NotRecorded);
  };
  Ok(match landed.attempt.asked_by() {
    MergeAsker::Person | MergeAsker::Retry { .. } => Approval::Person(merge.merged_at),
    MergeAsker::MergeQueue => db
      .merge_queue_entry(merge.run)?
      .and_then(|entry| entry.approved_at)
      .map_or(Approval::Automatic, Approval::Person),
    MergeAsker::Automation => Approval::Automatic,
  })
}

/// `run` of the project in `db` as an approved run; `None` when it was not merged and no person approved it in the
/// merge queue.
pub(super) fn approved_run(db: &ProjectDb, run: RunId) -> Result<Option<ApprovedRun>, StoreError> {
  if let Some(merge) = db.run_merge(run)? {
    return Ok(Some(ApprovedRun {
      approval: approval_of(db, &merge)?,
      landed: Some(Landed {
        into: merge.into,
        revision: merge.revision,
        kind: merge.kind,
        at: merge.merged_at,
      }),
    }));
  }
  let approved = db
    .merge_queue_entry(run)?
    .filter(|entry| entry.state.in_queue())
    .and_then(|entry| entry.approved_at);
  Ok(approved.map(|at| ApprovedRun {
    approval: Approval::Person(at),
    landed: None,
  }))
}
