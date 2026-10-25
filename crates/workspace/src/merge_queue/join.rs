//! Which runs join the queue: a task's latest run that passed every step of a pipeline ending in a Deliver step, as
//! the run pinned it, whose work was not merged and whose worktree is there. They join first finished first, at the
//! end of the queue. A pipeline without a Deliver step keeps today's way: its runs never join, and a person merges
//! them from the task page. A run that joined once does not join again, whatever became of it. A run that committed
//! nothing has nothing to merge and never joins (ORC-93): it is completed, or it waits for the person.

use std::path::Path;

use orchester_core::{integration::RunWork, store::RunStore, time::Timestamp};
use orchester_exec::programs::Programs;
use orchester_store::{ProjectDb, StoreError};

use crate::tasks::{
  delivery::{self, Delivery},
  detail::run_store_error,
};

/// Puts the project's runs that passed every step of a pipeline with a Deliver step in its queue at `now`, first
/// finished first, but those that committed nothing, as git in the checkout at `root` tells through `programs`;
/// whether any joined.
pub fn join(db: &mut ProjectDb, (root, programs): (&Path, &Programs), now: Timestamp) -> Result<bool, StoreError> {
  let candidates = db.merge_queue_candidates()?;
  let mut works = Vec::with_capacity(candidates.len());
  for (run, ..) in &candidates {
    let found = db.run(*run).map_err(run_store_error)?;
    works.push(found.as_ref().map(RunWork::of));
  }
  let known: Vec<RunWork> = works.iter().flatten().cloned().collect();
  let mut delivered = delivery::deliveries(programs, root, &known).into_iter();
  let mut joined = false;
  for ((run, task, finished), work) in candidates.into_iter().zip(works) {
    let nothing = work.is_some() && delivered.next() == Some(Delivery::NothingToMerge);
    if !nothing {
      joined |= db.join_merge_queue((run, task), finished, now)?;
    }
  }
  Ok(joined)
}
