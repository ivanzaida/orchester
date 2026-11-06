//! The merge queue's work in a run's worktree (protocol 41, ORC-684): the desktop drives the queue, and the runner,
//! which reaches the run's worktree, syncs the base branch into it and runs Check on the merged result there.
//! Landing the run, the merge into the person's checkout, stays the desktop's.

use orchester_core::{
  id::RunId,
  integration::delivery::Synced,
  merge_queue::recheck::{RecheckTool, Rechecked},
  run::BranchName,
};
use serde::{Deserialize, Serialize};

/// What the merge queue asks of a run's worktree ([`crate::Command::MergeQueue`]).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum MergeQueueCommand {
  /// Merges `base`, as the repository has it now, into `branch`, the run's branch checked out in run `run`'s
  /// worktree; a merge that conflicts is put back. Answered with [`MergeQueueReply::Synced`], or
  /// [`MergeQueueReply::SyncFailed`] when git could not do it.
  SyncBase {
    run: RunId,
    base: BranchName,
    branch: BranchName,
  },
  /// Runs `tools`, in order, in run `run`'s worktree, as its tool steps run them, until one does not pass. Answered
  /// with [`MergeQueueReply::Rechecked`].
  Recheck { run: RunId, tools: Vec<RecheckTool> },
}

/// The answer to a [`MergeQueueCommand`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum MergeQueueReply {
  /// How syncing went.
  Synced(Synced),
  /// Git could not sync; why.
  SyncFailed { detail: String },
  /// How Check went on the merged result.
  Rechecked(Rechecked),
}
