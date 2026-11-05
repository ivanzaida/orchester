//! *Approve and merge* at the head the person reviewed: through the merge queue's approval when the run waits for it
//! there (`WaitingForApproval`; the queue merges it in its turn), else the task page's merge with `expected` set to
//! that head, which merges nothing once the branch moved. Unavailable while Implement works on comments sent to it. A
//! branch that moved after the reviewed head is *Changed since your review*: nothing merges, and the run is reviewed
//! again. Blocks on SQLite and git.

use std::path::Path;

use orchester_core::{
  id::RunId, integration::branch_head, merge_queue::QueueState, revision::Revision, run::Answerer, time::Timestamp,
};
use orchester_exec::programs::Programs;
use orchester_store::ProjectDb;

use super::{ReviewRefusal, implement_at_work, recover, waiting};
use crate::{
  merge_queue,
  tasks::{
    changes::GIT,
    integration::{IntegrationProblem, Merged, merge_in},
    worktree_removal::WorktreeRemover,
  },
};

/// How an approval went on.
#[derive(Debug)]
pub enum Approved {
  /// Merged now, from the task page.
  Merged(Box<Merged>),
  /// Approved in the merge queue, which merges the run in its turn.
  Queued,
}

/// The person approves `run` of the project whose run worktrees are `worktrees` at `reviewed`, the head they reviewed,
/// at `now`, and it merges there. `programs` read the run's branch and merge it; the project's runner removes what
/// the run left.
pub fn approve_and_merge(
  worktrees: &WorktreeRemover,
  programs: &Programs,
  (run, reviewed): (RunId, &Revision),
  now: Timestamp,
) -> Result<Approved, ReviewRefusal> {
  let root = worktrees.root();
  let mut db = ProjectDb::open(root)?;
  let threads = recover::threads(&mut db, run)?;
  let stored = match waiting(&db, run) {
    Ok(stored) => stored,
    Err(ReviewRefusal::NotWaiting) if threads.iter().any(|thread| thread.sent().is_some()) => {
      return Err(ReviewRefusal::ImplementAtWork);
    }
    Err(refusal) => return Err(refusal),
  };
  if implement_at_work(&threads, &stored) {
    return Err(ReviewRefusal::ImplementAtWork);
  }
  let head = branch_head(&programs.runner(), Path::new(GIT), (root, stored.branch()))
    .map_err(|error| ReviewRefusal::Merge(IntegrationProblem::Read(error)))?;
  if head.as_ref() != Some(reviewed) {
    return Err(ReviewRefusal::ChangedSinceReview);
  }
  let in_queue = db
    .merge_queue_entry(run)?
    .is_some_and(|entry| entry.state == QueueState::WaitingForApproval);
  drop(db);
  if in_queue {
    merge_queue::approve(root, run, now, Answerer::Person).map_err(ReviewRefusal::Queue)?;
    return Ok(Approved::Queued);
  }
  Ok(Approved::Merged(Box::new(merge_in(
    worktrees,
    programs,
    (run, Some(reviewed)),
  )?)))
}
