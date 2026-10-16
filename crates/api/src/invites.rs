//! Invites a team server makes (`docs/architecture/server.md`, "Invites", "Codes"): by email through Orchester Cloud,
//! or by a code without the internet.

use orchester_core::id::{InviteId, ProjectId, UserId};
use serde::{Deserialize, Serialize};

use crate::{Credential, MemberRole, UserRef};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InviteKind {
  /// An account on the server with no project yet (admins).
  Server,
  /// A membership of one project with a role (its owners and admins).
  Project,
  /// A new device for an existing account (admins): an account code, or linking the account's cloud identity.
  Account,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InviteDelivery {
  Cloud,
  Code,
}

/// `POST /invites`.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InviteCreate {
  pub kind: InviteKind,
  pub delivery: InviteDelivery,
  pub email: Option<String>,
  pub project: Option<ProjectId>,
  pub role: Option<MemberRole>,
  pub user: Option<UserId>,
  /// A server invite that makes the new account an admin.
  #[serde(default)]
  pub admin: bool,
}

/// Where an invite stands. Its email is forgotten once it is not `open`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InviteState {
  Open,
  Accepted,
  Expired,
  Revoked,
}

/// An invite. Never carries its code.
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
  /// When it was accepted or revoked; an expired invite ended at `expires_at`.
  #[serde(default)]
  pub ended_at: Option<i64>,
  /// Who accepted or revoked it.
  #[serde(default)]
  pub ended_by: Option<UserRef>,
}

const fn open() -> InviteState {
  InviteState::Open
}

/// `200` of `DELETE /invites/{id}`: for an invite by email, the revocation the server signed for the cloud, which the
/// server admin's page delivers.
#[derive(Debug, Serialize, Deserialize)]
pub struct InviteRevoked {
  pub revocation: Option<String>,
}

/// `201` of `POST /invites`. `code` only for `delivery: code` and `signed` only for `delivery: cloud`, each shown
/// once: the signed invite is what the server admin's page carries to the cloud when the server cannot reach it.
#[derive(Debug, Serialize, Deserialize)]
pub struct InviteCreated {
  pub invite: Invite,
  pub code: Option<Credential>,
  pub signed: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Invites {
  pub invites: Vec<Invite>,
}
