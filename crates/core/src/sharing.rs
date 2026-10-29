//! Pure shared-task transitions. Storage serializes each transition with its task.

mod cached;
mod delta;
mod similar;

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub use self::{
  cached::{CachedClaim, CachedRunStatus, CachedTask, SharedPerson},
  delta::{DeltaOutcome, RunProgress, TagRemap, TaskChange, TaskDelta, TaskDeltaKind, remap_tags},
  similar::similar_titles,
};
use crate::{
  id::{ClaimId, UserId},
  run::TaskPhase,
  time::Timestamp,
};

pub const CLAIM_STALE_MS: i64 = 15 * 60 * 1000;

/// A member's role in a server project: owners (and the server's admins) manage its members.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemberRole {
  Owner,
  Member,
}

/// Whether a person may change a shared project's team configuration (pipelines, agent profiles, instructions,
/// skills, tools): only its owner or the server's admin, who may not be a member at all (server.md, row 72).
pub const fn may_change_configuration(role: Option<MemberRole>, server_admin: bool) -> bool {
  server_admin || matches!(role, Some(MemberRole::Owner))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimKind {
  Manual,
  Run,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Claim {
  id: ClaimId,
  holder: UserId,
  kind: ClaimKind,
  since: Timestamp,
  heartbeat_at: Timestamp,
  taken_over_from: Option<UserId>,
}

#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum ClaimError {
  #[error("task is already held")]
  Held,
  #[error("task is merged")]
  Merged,
  #[error("claim belongs to another member")]
  NotHolder,
  #[error("claim changed")]
  Changed,
  #[error("claim is still fresh")]
  NotStale,
  #[error("stop the active agent before releasing the claim")]
  AgentWorking,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClaimEnd {
  Keep,
  Release,
  Complete,
}

pub const fn claim_end(phase: TaskPhase) -> ClaimEnd {
  match phase {
    TaskPhase::Cancelled => ClaimEnd::Release,
    TaskPhase::Merged => ClaimEnd::Complete,
    _ => ClaimEnd::Keep,
  }
}

impl Claim {
  pub fn take(
    current: Option<&Self>,
    id: ClaimId,
    holder: UserId,
    kind: ClaimKind,
    now: Timestamp,
    merged: bool,
  ) -> Result<Self, ClaimError> {
    if merged {
      return Err(ClaimError::Merged);
    }
    if let Some(current) = current {
      if current.holder != holder {
        return Err(ClaimError::Held);
      }
      let mut claim = current.clone();
      claim.heartbeat_at = now;
      if kind == ClaimKind::Run {
        claim.kind = ClaimKind::Run;
      }
      return Ok(claim);
    }
    Ok(Self {
      id,
      holder,
      kind,
      since: now,
      heartbeat_at: now,
      taken_over_from: None,
    })
  }

  pub fn heartbeat(&mut self, holder: UserId, now: Timestamp) -> Result<(), ClaimError> {
    if self.holder != holder {
      return Err(ClaimError::NotHolder);
    }
    self.heartbeat_at = now;
    Ok(())
  }

  pub fn take_over(&self, seen: ClaimId, new_id: ClaimId, holder: UserId, now: Timestamp) -> Result<Self, ClaimError> {
    if self.id != seen {
      return Err(ClaimError::Changed);
    }
    if !self.stale(now) {
      return Err(ClaimError::NotStale);
    }
    Ok(Self {
      id: new_id,
      holder,
      kind: self.kind,
      since: now,
      heartbeat_at: now,
      taken_over_from: Some(self.holder),
    })
  }

  pub fn check_release(&self, holder: UserId, phase: Option<TaskPhase>) -> Result<(), ClaimError> {
    if self.holder != holder {
      return Err(ClaimError::NotHolder);
    }
    if phase.is_some_and(TaskPhase::is_active) {
      return Err(ClaimError::AgentWorking);
    }
    Ok(())
  }

  pub fn stale(&self, now: Timestamp) -> bool {
    now.unix_millis().saturating_sub(self.heartbeat_at.unix_millis()) >= CLAIM_STALE_MS
  }

  pub const fn id(&self) -> ClaimId {
    self.id
  }
  pub const fn holder(&self) -> UserId {
    self.holder
  }
  pub const fn kind(&self) -> ClaimKind {
    self.kind
  }
  pub const fn since(&self) -> Timestamp {
    self.since
  }
  pub const fn heartbeat_at(&self) -> Timestamp {
    self.heartbeat_at
  }
  pub const fn taken_over_from(&self) -> Option<UserId> {
    self.taken_over_from
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "u64", into = "u64")]
pub struct TaskRevision(u64);

#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
#[error("task revision is out of range")]
pub struct RevisionError;

impl TaskRevision {
  pub const FIRST: Self = Self(1);
  pub fn next(self) -> Result<Self, RevisionError> {
    self.0.checked_add(1).ok_or(RevisionError).and_then(Self::try_from)
  }
  pub const fn get(self) -> u64 {
    self.0
  }
}

impl TryFrom<u64> for TaskRevision {
  type Error = RevisionError;
  fn try_from(value: u64) -> Result<Self, Self::Error> {
    if value == 0 || value > i64::MAX as u64 {
      return Err(RevisionError);
    }
    Ok(Self(value))
  }
}
impl From<TaskRevision> for u64 {
  fn from(value: TaskRevision) -> Self {
    value.0
  }
}
