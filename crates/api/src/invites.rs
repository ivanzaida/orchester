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
  /// A membership of one workspace with a role (created by workspace or server admins).
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

/// Why the team server could not reach Orchester Cloud to deliver a signed invite or revocation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CloudCause {
  /// No connection: no route, the name did not resolve, or the connection was reset.
  Unreachable,
  /// The cloud's certificate was not trusted, or the TLS handshake failed.
  Tls,
  /// The connection was refused.
  Refused,
  /// No answer in time.
  Timeout,
}

/// How the team server's own delivery of a signed invite or revocation to Orchester Cloud went.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CloudDelivery {
  /// The cloud took it.
  Delivered,
  /// The cloud answered with another status than the contract's.
  Declined,
  /// The server has no cloud configured: the invite was made but nobody delivered it.
  NoCloud,
  /// The server could not reach the cloud; the server admin's page offers to send it from the person's browser.
  Unreachable { cause: CloudCause },
}

/// `200` of `DELETE /invites/{id}`: for an invite by email, the revocation the server signed for the cloud and how the
/// server's own delivery of it went; when the server could not reach the cloud, the server admin's page delivers it.
#[derive(Debug, Serialize, Deserialize)]
pub struct InviteRevoked {
  pub revocation: Option<String>,
  /// `None` for an invite by code, and from a server that does not deliver revocations itself.
  #[serde(default)]
  pub cloud_delivery: Option<CloudDelivery>,
}

/// `201` of `POST /invites`. `code` only for `delivery: code` and `signed` only for `delivery: cloud`, each shown
/// once. The server delivers the signed invite to the cloud itself and says how in `cloud_delivery`; when it could not
/// reach the cloud, the server admin's page carries `signed` there from the person's browser.
#[derive(Debug, Serialize, Deserialize)]
pub struct InviteCreated {
  pub invite: Invite,
  pub code: Option<Credential>,
  pub signed: Option<String>,
  /// `None` for an invite by code, and from a server that does not deliver invites itself.
  #[serde(default)]
  pub cloud_delivery: Option<CloudDelivery>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Invites {
  pub invites: Vec<Invite>,
}
