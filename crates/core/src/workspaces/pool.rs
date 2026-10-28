//! Which workspace a borrower takes. A project has workspaces `1..=N`, N its agents at once; a workspace above N
//! (after N was lowered) is never taken again, stays idle and is cleaned first. A borrower takes, of the free ones
//! within N, the one its run used last, else the one idle longest, else a new one up to N; with none it waits, and
//! says how many of the N are in use. A workspace a run left uncommitted changes in is stranded: nobody takes it
//! until a person keeps those changes in a branch or discards them.

use std::num::NonZeroU32;

use serde::{Deserialize, Serialize};

use super::WorkspaceNumber;
use crate::{
  id::{RunId, StepExecutionId},
  run::BranchName,
  time::Timestamp,
};

/// Who works in a workspace.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Holder {
  /// A run whose steps follow one another, on its branch.
  Run { run: RunId },
  /// A step of the run that works beside others, on a branch of its own off the run's.
  Step { run: RunId, execution: StepExecutionId },
  /// The merge queue re-checking the run before it lands.
  Recheck { run: RunId },
  /// The project's warm-up command, on the base branch.
  WarmUp,
}

impl Holder {
  /// The run it works for; `None` for the warm-up.
  pub const fn run(self) -> Option<RunId> {
    match self {
      Self::Run { run } | Self::Step { run, .. } | Self::Recheck { run } => Some(run),
      Self::WarmUp => None,
    }
  }
}

/// Whether a workspace holds anything built: a workspace is cold until something worked in it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Warmth {
  Cold,
  Warm,
}

/// The uncommitted changes a run that stopped unexpectedly left in a workspace, which could not be switched to
/// another branch without losing them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stranded {
  /// The run that worked there last; `None` when no run did, for changes made there by hand.
  pub run: Option<RunId>,
  /// Files with changes, untracked ones included.
  pub files: u32,
}

/// A workspace as the store keeps it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkspaceRecord {
  pub number: WorkspaceNumber,
  /// Who works there now; `None` while it is idle.
  pub holder: Option<Holder>,
  /// When it was last taken or given back: since when it is held, or idle.
  pub last_used: Option<Timestamp>,
  /// The run that worked there last, which takes it first.
  pub last_run: Option<RunId>,
  /// The branch checked out there last; `None` for the base branch, or a workspace never used.
  pub branch: Option<BranchName>,
  pub warmth: Warmth,
  pub stranded: Option<Stranded>,
}

impl WorkspaceRecord {
  /// A workspace that was just made: cold, idle and never used.
  pub const fn new(number: WorkspaceNumber) -> Self {
    Self {
      number,
      holder: None,
      last_used: None,
      last_run: None,
      branch: None,
      warmth: Warmth::Cold,
      stranded: None,
    }
  }

  /// Whether a borrower may take it with `limit` agents at once: idle, not stranded, and within the limit.
  pub fn is_free(&self, limit: NonZeroU32) -> bool {
    self.holder.is_none() && self.stranded.is_none() && self.number.get() <= limit.get()
  }

  /// How the workspace shows in Project settings › Runs with `limit` agents at once.
  pub fn state(&self, limit: NonZeroU32) -> WorkspaceState {
    match (self.holder, self.stranded) {
      (Some(Holder::WarmUp), _) => WorkspaceState::WarmingUp,
      (Some(holder), _) => WorkspaceState::InUse(holder),
      (None, Some(stranded)) => WorkspaceState::Stranded(stranded),
      (None, None) if self.number.get() > limit.get() => WorkspaceState::OverLimit,
      (None, None) => WorkspaceState::Idle(self.warmth),
    }
  }
}

/// What a workspace is doing, as people see it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkspaceState {
  InUse(Holder),
  WarmingUp,
  /// A run left uncommitted changes; a person keeps them in a branch or discards them.
  Stranded(Stranded),
  /// Above the agents at once since they were lowered: idle, never taken, cleaned first.
  OverLimit,
  Idle(Warmth),
}

/// Why a borrower waits: every workspace it may take is in use.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WaitReason {
  /// Workspaces within the limit that are held or stranded.
  pub in_use: u32,
  /// The agents at once: how many workspaces there may be.
  pub of: u32,
}

/// What a borrower gets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Borrow {
  /// This workspace, made first when no record of it exists yet.
  Take(WorkspaceNumber),
  Wait(WaitReason),
}

/// The workspace `holder` takes among `records` with `limit` agents at once: the one it holds already, else the one
/// its run used last that is free (the latest of them, should a run have used several), else the free one idle
/// longest, else the lowest number up to the limit that has no record yet. With none, it waits.
pub fn choose(records: &[WorkspaceRecord], limit: NonZeroU32, holder: Holder) -> Borrow {
  if let Some(held) = records.iter().find(|record| record.holder == Some(holder)) {
    return Borrow::Take(held.number);
  }
  let free = || records.iter().filter(|record| record.is_free(limit));
  let last = holder.run().and_then(|run| {
    free()
      .filter(|record| record.last_run == Some(run))
      .max_by_key(|record| (record.last_used, std::cmp::Reverse(record.number)))
  });
  if let Some(last) = last {
    return Borrow::Take(last.number);
  }
  if let Some(idle) = free().min_by_key(|record| (record.last_used, record.number)) {
    return Borrow::Take(idle.number);
  }
  if let Some(new) = WorkspaceNumber::up_to(limit).find(|number| !records.iter().any(|record| record.number == *number))
  {
    return Borrow::Take(new);
  }
  let in_use = records
    .iter()
    .filter(|record| record.number.get() <= limit.get() && !record.is_free(limit))
    .count();
  Borrow::Wait(WaitReason {
    in_use: u32::try_from(in_use).unwrap_or(u32::MAX),
    of: limit.get(),
  })
}
