//! Which runs join the queue: a task's latest run that passed every step of a pipeline ending in a Deliver step, as
//! the run pinned it, whose work was not merged and whose worktree is there. They join first finished first, at the
//! end of the queue. A pipeline without a Deliver step keeps today's way: its runs never join, and a person merges
//! them from the task page. A run that joined once does not join again, whatever became of it.

use orchester_core::time::Timestamp;
use orchester_store::{ProjectDb, StoreError};

/// Puts the project's runs that passed every step of a pipeline with a Deliver step in its queue at `now`, first
/// finished first; whether any joined.
pub fn join(db: &mut ProjectDb, now: Timestamp) -> Result<bool, StoreError> {
  let mut joined = false;
  for (run, task, finished) in db.merge_queue_candidates()? {
    joined |= db.join_merge_queue((run, task), finished, now)?;
  }
  Ok(joined)
}
