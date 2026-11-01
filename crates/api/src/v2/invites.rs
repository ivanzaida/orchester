use orchester_core::id::{InviteId, ProjectId, UserId};
use serde::{Deserialize, Serialize};

use super::MemberRole;
use crate::{CloudDelivery, Credential, InviteDelivery, InviteKind, InviteState, UserRef};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InviteCreate {
  pub kind: InviteKind,
  pub delivery: InviteDelivery,
  pub email: Option<String>,
  pub project: Option<ProjectId>,
  pub role: Option<MemberRole>,
  pub user: Option<UserId>,
  /// Only server invites can grant the separate server-admin account attribute.
  #[serde(default)]
  pub admin: bool,
}

/// Invite metadata for the new surface. Existing signed cloud-invite contracts are not changed.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Invite {
  pub id: InviteId,
  pub kind: InviteKind,
  pub email: Option<String>,
  pub project: Option<ProjectId>,
  pub role: Option<MemberRole>,
  pub user: Option<UserRef>,
  pub admin: bool,
  pub by: UserRef,
  pub delivery: InviteDelivery,
  pub expires_at: i64,
  #[serde(default)]
  pub created_at: i64,
  #[serde(default = "open")]
  pub state: InviteState,
  #[serde(default)]
  pub ended_at: Option<i64>,
  #[serde(default)]
  pub ended_by: Option<UserRef>,
}

const fn open() -> InviteState {
  InviteState::Open
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InviteCreated {
  pub invite: Invite,
  pub code: Option<Credential>,
  pub signed: Option<String>,
  #[serde(default)]
  pub cloud_delivery: Option<CloudDelivery>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Invites {
  pub invites: Vec<Invite>,
}
