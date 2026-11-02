use orchester_core::sharing::{WorkspaceRole, may_administer_workspace, may_read_workspace};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Optimistic concurrency token for workspace policy, independent of task revisions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "u64", into = "u64")]
pub struct PolicyRevision(u64);

#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
#[error("workspace policy revision is out of range")]
pub struct PolicyRevisionError;

impl PolicyRevision {
  pub const FIRST: Self = Self(1);

  pub const fn get(self) -> u64 {
    self.0
  }

  pub fn next(self) -> Result<Self, PolicyRevisionError> {
    self
      .0
      .checked_add(1)
      .ok_or(PolicyRevisionError)
      .and_then(Self::try_from)
  }
}

impl TryFrom<u64> for PolicyRevision {
  type Error = PolicyRevisionError;

  fn try_from(value: u64) -> Result<Self, Self::Error> {
    if value == 0 || value > i64::MAX as u64 {
      return Err(PolicyRevisionError);
    }
    Ok(Self(value))
  }
}

impl From<PolicyRevision> for u64 {
  fn from(value: PolicyRevision) -> Self {
    value.get()
  }
}

/// Effective persisted policy. New workspaces start at revision 1 with automatic merge off.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspacePolicy {
  pub revision: PolicyRevision,
  pub automatic_merge: bool,
}

impl Default for WorkspacePolicy {
  fn default() -> Self {
    Self {
      revision: PolicyRevision::FIRST,
      automatic_merge: false,
    }
  }
}

/// Read-only caller capabilities; never accept these as authorization input.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceCapabilities {
  pub read_policy: bool,
  pub change_policy: bool,
}

impl WorkspaceCapabilities {
  /// Resolve from authoritative membership and server-admin records for this workspace.
  pub const fn for_caller(role: Option<WorkspaceRole>, server_admin: bool) -> Self {
    Self {
      read_policy: may_read_workspace(role, server_admin),
      change_policy: may_administer_workspace(role, server_admin),
    }
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspacePolicyResponse {
  pub policy: WorkspacePolicy,
  pub capabilities: WorkspaceCapabilities,
}

/// Replace automatic_merge only if revision still matches; authorize before applying in one transaction.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspacePolicyChanges {
  pub revision: PolicyRevision,
  pub automatic_merge: bool,
}

/// HTTP 409 on the versioned surface; current policy lets a caller review and retry.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "error", rename_all = "snake_case", deny_unknown_fields)]
pub enum WorkspacePolicyError {
  #[error("workspace policy revision changed")]
  PolicyRevisionConflict { current: WorkspacePolicy },
}
