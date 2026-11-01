use orchester_core::id::{ProjectId, UserId};
use serde::{Deserialize, Serialize};

use super::MemberRole;
use crate::{Delta, ServerAdmin, UserRef};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Member {
  pub user: UserRef,
  #[serde(default)]
  pub username: String,
  pub role: MemberRole,
  pub added_at: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Members {
  pub members: Vec<Member>,
  /// Server admins inherit workspace access without a workspace membership.
  #[serde(default)]
  pub admins: Vec<ServerAdmin>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MemberCreate {
  pub username: String,
  pub role: MemberRole,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MemberChanges {
  pub role: MemberRole,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MemberResponse {
  pub member: Member,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MemberAdded {
  pub member: Member,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemberRoleChanged {
  pub user: UserId,
  pub role: MemberRole,
}

/// Only membership deltas on the new surface; the existing full event stream retains its original types.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data")]
pub enum MembershipEvent {
  #[serde(rename = "member.added")]
  Added(Delta<MemberAdded>),
  #[serde(rename = "member.role_changed")]
  RoleChanged(Delta<MemberRoleChanged>),
  #[serde(rename = "member.removed")]
  Removed(Delta<crate::MemberRemoved>),
}

/// Workspace-specific failure on the new surface. Existing server-admin and v1 error codes are unchanged.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MembershipErrorCode {
  LastWorkspaceAdmin,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged, deny_unknown_fields)]
pub enum MembershipErrorDetails {
  Remote {
    project: ProjectId,
    member: bool,
    admins: Vec<UserRef>,
  },
  Admins {
    admins: Vec<UserRef>,
  },
}
