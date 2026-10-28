//! Cleaning workspaces when space is short: deleting a workspace's warm folders (its branch, checkpoints and the
//! run's artifacts stay; the next run in it builds from cold). Workspaces over the limit go first, then the ones idle
//! longest. One in use, or holding a run's uncommitted changes, is never cleaned.

use std::num::NonZeroU32;

use super::{WorkspaceNumber, pool::WorkspaceRecord};
use crate::id::RunId;

/// The workspaces cleanup takes, in its order: idle ones over `limit` first, then idle ones within it, each group the
/// one idle longest first.
pub fn cleanup_order(records: &[WorkspaceRecord], limit: NonZeroU32) -> Vec<WorkspaceNumber> {
  let mut idle: Vec<&WorkspaceRecord> = records
    .iter()
    .filter(|record| record.holder.is_none() && record.stranded.is_none())
    .collect();
  idle.sort_by_key(|record| (record.number.get() <= limit.get(), record.last_used, record.number));
  idle.into_iter().map(|record| record.number).collect()
}

/// A workspace the Clean up question lists, and whether it starts checked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CleanupChoice {
  pub number: WorkspaceNumber,
  /// `false` for a workspace whose last run goes on (paused, waiting): its next step would rebuild.
  pub checked: bool,
}

/// The workspaces Clean up offers, in [`cleanup_order`], checked unless the run that used it last is one of
/// `continuing`, the runs that have not ended.
pub fn default_cleanup(records: &[WorkspaceRecord], limit: NonZeroU32, continuing: &[RunId]) -> Vec<CleanupChoice> {
  cleanup_order(records, limit)
    .into_iter()
    .map(|number| {
      let last_run = records
        .iter()
        .find(|record| record.number == number)
        .and_then(|record| record.last_run);
      CleanupChoice {
        number,
        checked: last_run.is_none_or(|run| !continuing.contains(&run)),
      }
    })
    .collect()
}
