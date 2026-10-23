//! A project's merge queue as the project store keeps it (`docs/design/orchester-workspace.md` › *Delivery*): one
//! entry per run that passed every step of a pipeline ending in a Deliver step, where it stands in delivering it into
//! the base branch, and what happened to it on the way ([`QueueEvent`]). The order, the moves from state to state and
//! every rule of delivery are `orchester-workspace`'s (`merge_queue`); these are only its records.

use serde::{Deserialize, Serialize};

use crate::{
  id::{RunId, TaskId},
  integration::MergeKind,
  resource::StepKey,
  revision::Revision,
  time::Timestamp,
};

/// How many times in a row a run syncs again because its base branch moved while Check ran on the merged result,
/// before it leaves the queue with the reason that the base keeps moving.
pub const MAX_RESYNCS: u32 = 3;

/// Where a run stands in the queue.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum QueueState {
  /// It waits for its turn.
  Queued,
  /// The current base branch is merged into the run's branch, in its worktree.
  Syncing,
  /// Check runs again on the merged result: the run's branch at `tip`, holding the base branch at `base`.
  Rechecking { base: Revision, tip: Revision },
  /// The run's work went back to its step `step`, the Integrate step, to resolve `files`, which conflicted with the
  /// base branch at `base`; the run's last event was `after` when it went.
  Integrating {
    base: Revision,
    step: StepKey,
    files: Vec<String>,
    after: u64,
  },
  /// Check passed on the merged result; the run waits for a person to review it and merge. It does not hold the
  /// queue.
  WaitingForApproval,
  /// Delivery cannot go on until what [`Blocked`] names changes; it holds the whole queue.
  Blocked(Blocked),
  /// The run left the queue; nothing of it reached the base branch.
  Left(Left),
  /// The run's work is on the base branch.
  Merged(Delivered),
  /// A person took the run out of the queue: it is ready for review again, merged from its task page, and does not
  /// join the queue again.
  Removed,
}

impl QueueState {
  /// Whether the run is still in the queue: not merged and not gone from it.
  pub const fn in_queue(&self) -> bool {
    !matches!(self, Self::Left(_) | Self::Merged(_) | Self::Removed)
  }

  /// Whether the entry is being delivered now: synced, checked, integrated, blocked on its way to the base branch.
  pub const fn delivering(&self) -> bool {
    matches!(
      self,
      Self::Syncing | Self::Rechecking { .. } | Self::Integrating { .. } | Self::Blocked(_)
    )
  }
}

/// Why delivery waits.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "blocked", rename_all = "snake_case")]
pub enum Blocked {
  /// The base branch is checked out in the person's checkout with these uncommitted files (`git status` lines, the
  /// status letter first): Orchester never merges into such a checkout.
  Uncommitted { files: Vec<String> },
  /// The checkout has another branch checked out (`None`: a detached HEAD), so the base branch cannot be brought in.
  OtherBranch { current: Option<String> },
  /// A merge, cherry-pick or revert is in progress in the checkout.
  InProgress,
  /// A local integration holds the checkout.
  HeldLocally,
}

/// Why a run left the queue.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "left", rename_all = "snake_case")]
pub enum Left {
  /// The Integrate step could not resolve these files, or no step can: the task waits for a person.
  Conflicts { files: Vec<String> },
  /// Check failed on the merged result, with its tool's name and the end of what it printed.
  CheckFailed { tool: String, detail: String },
  /// The base branch moved during each of the last [`MAX_RESYNCS`] checks; the heads it moved to.
  KeepsMoving { base: String, heads: Vec<Revision> },
  /// Delivery could not go on, and why, in git's or the store's words.
  Stopped { detail: String },
  /// The task has a later run: this one is not delivered.
  LaterRun,
}

impl Left {
  /// Whether this is a failure (shown red) rather than a person's decision the run waits for.
  pub const fn failed(&self) -> bool {
    !matches!(self, Self::KeepsMoving { .. })
  }
}

/// How a run reached the base branch, and what was cleared after it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Delivered {
  pub revision: Revision,
  pub kind: MergeKind,
  /// The run's worktrees and its code index went.
  pub worktree_removed: bool,
  /// The run's merged branch was deleted.
  pub branch_deleted: bool,
}

/// The base branch and the run's branch a check passed on.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckedOn {
  pub base: Revision,
  pub tip: Revision,
  pub at: Timestamp,
}

/// One run in the queue.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QueueEntry {
  pub run: RunId,
  pub task: TaskId,
  /// When the run passed every step: the queue delivers first finished first.
  pub finished_at: Timestamp,
  /// Its place in the queue's order; lower goes first.
  pub position: i64,
  pub state: QueueState,
  /// When it came to its state.
  pub since: Timestamp,
  /// How many times in a row it synced again because the base branch moved while it was checked.
  pub resyncs: u32,
  /// The last check on the merged result that passed.
  pub checked: Option<CheckedOn>,
  /// When a person approved the merge; a run of an Auto-merge pipeline is never approved.
  pub approved_at: Option<Timestamp>,
}

/// What happened to a run on its way through the queue, for its Activity and the task's conversation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum QueueEvent {
  Joined,
  /// The base branch at `base` was merged into the run's branch; `files` conflicted (none: it merged cleanly).
  Synced { base: Revision, files: Vec<String> },
  /// The conflicts went to the Integrate step `step`.
  IntegrateStarted { step: StepKey, files: Vec<String> },
  CheckStarted { base: Revision },
  CheckPassed { base: Revision, tool: String },
  CheckFailed { base: Revision, tool: String },
  /// The base branch moved from `base` to `now` while Check ran: the check no longer matches what would land.
  CheckSuperseded { base: Revision, now: Revision },
  Approved,
  Blocked(Blocked),
  Merged { revision: Revision, kind: MergeKind },
  CleanedUp { worktree_removed: bool, branch_deleted: bool },
  Left(Left),
  /// A person took the run out of the queue; it is ready for review again.
  Removed,
  /// A person resolved what stopped it and it joined the queue again.
  Requeued,
}

/// A [`QueueEvent`] of a run, and when it happened.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecordedQueueEvent {
  pub run: RunId,
  pub at: Timestamp,
  pub event: QueueEvent,
}
