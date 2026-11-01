use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Membership of one workspace. Server administration is a separate, authenticated account attribute.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemberRole {
  Admin,
  Member,
}

/// Workspace admins inherit member access; server admins inherit access without artificial membership.
pub const fn may_read_workspace(role: Option<MemberRole>, server_admin: bool) -> bool {
  server_admin || role.is_some()
}

/// The role and server-admin flag must come from authoritative identity and membership records, not a request.
pub const fn may_administer_workspace(role: Option<MemberRole>, server_admin: bool) -> bool {
  server_admin || matches!(role, Some(MemberRole::Admin))
}

/// Only workspace admins and server admins change team pipelines, profiles, instructions, skills and tools.
pub const fn may_change_configuration(role: Option<MemberRole>, server_admin: bool) -> bool {
  may_administer_workspace(role, server_admin)
}

#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum MembershipChangeError {
  #[error("a workspace must retain at least one admin")]
  LastAdmin,
}

/// Checks removal (`None`) or demotion against the current workspace-admin count in the same transaction.
/// Server admins do not count as workspace memberships and do not replace the last workspace admin.
pub const fn check_workspace_admin_change(
  current: MemberRole,
  next: Option<MemberRole>,
  admins: usize,
) -> Result<(), MembershipChangeError> {
  if matches!(current, MemberRole::Admin) && !matches!(next, Some(MemberRole::Admin)) && admins <= 1 {
    Err(MembershipChangeError::LastAdmin)
  } else {
    Ok(())
  }
}
