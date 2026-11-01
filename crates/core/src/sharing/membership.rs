use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::MemberRole;

/// Membership of one workspace. Server administration is a separate, authenticated account attribute.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceRole {
  Admin,
  Member,
}

/// Workspace admins inherit member access; server admins inherit access without artificial membership.
pub const fn may_read_workspace(role: Option<WorkspaceRole>, server_admin: bool) -> bool {
  server_admin || role.is_some()
}

/// The role and server-admin flag must come from authoritative identity and membership records, not a request.
pub const fn may_administer_workspace(role: Option<WorkspaceRole>, server_admin: bool) -> bool {
  server_admin || matches!(role, Some(WorkspaceRole::Admin))
}

/// An explicit boundary adapter; the existing contract retains its Owner/Member names and encoding.
impl From<MemberRole> for WorkspaceRole {
  fn from(role: MemberRole) -> Self {
    match role {
      MemberRole::Owner => Self::Admin,
      MemberRole::Member => Self::Member,
    }
  }
}

impl From<WorkspaceRole> for MemberRole {
  fn from(role: WorkspaceRole) -> Self {
    match role {
      WorkspaceRole::Admin => Self::Owner,
      WorkspaceRole::Member => Self::Member,
    }
  }
}

#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum MembershipChangeError {
  #[error("a workspace must retain at least one admin")]
  LastAdmin,
}

/// Checks removal (`None`) or demotion against the current workspace-admin count in the same transaction.
/// Server admins do not count as workspace memberships and do not replace the last workspace admin.
pub const fn check_workspace_admin_change(
  current: WorkspaceRole,
  next: Option<WorkspaceRole>,
  admins: usize,
) -> Result<(), MembershipChangeError> {
  if matches!(current, WorkspaceRole::Admin) && !matches!(next, Some(WorkspaceRole::Admin)) && admins <= 1 {
    Err(MembershipChangeError::LastAdmin)
  } else {
    Ok(())
  }
}
