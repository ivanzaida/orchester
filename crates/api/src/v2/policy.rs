use orchester_core::sharing::{WorkspaceRole, may_administer_workspace, may_read_workspace};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::LimitRaises;
use crate::Delta;

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

/// Effective persisted policy. New workspaces start at revision 1 with automatic merge off and no raise of a run's
/// limits by its holder.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspacePolicy {
  pub revision: PolicyRevision,
  pub automatic_merge: bool,
  /// The Admin's ceiling on raises of a team run's limits by the claim's holder (OA-34); `None`, left out, is off.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub limit_raises: Option<LimitRaises>,
}

impl Default for WorkspacePolicy {
  fn default() -> Self {
    Self {
      revision: PolicyRevision::FIRST,
      automatic_merge: false,
      limit_raises: None,
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

/// Replace automatic_merge, and the ceiling of limit raises when named, only if revision still matches; authorize
/// before applying in one transaction.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspacePolicyChanges {
  pub revision: PolicyRevision,
  pub automatic_merge: bool,
  /// The ceiling of limit raises (OA-34), `null` to turn it off; left out, it stays as it is. A server older than it
  /// refuses it.
  #[serde(
    default,
    deserialize_with = "crate::tasks::present",
    skip_serializing_if = "Option::is_none"
  )]
  pub limit_raises: Option<Option<LimitRaises>>,
}

/// HTTP 409 on the versioned surface; current policy lets a caller review and retry.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "error", rename_all = "snake_case", deny_unknown_fields)]
pub enum WorkspacePolicyError {
  #[error("workspace policy revision changed")]
  PolicyRevisionConflict { current: WorkspacePolicy },
}

/// The workspace's new effective policy after a change; a client replaces its cached policy with it, and ignores the
/// event when `policy.revision` is not newer than the revision it holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyChanged {
  pub policy: WorkspacePolicy,
}

/// Workspace policy deltas on the new surface; the existing full event stream retains its original types.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data")]
pub enum WorkspacePolicyEvent {
  #[serde(rename = "policy.changed")]
  Changed(Delta<PolicyChanged>),
}
