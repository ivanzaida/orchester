//! How a device joins a team server in API 3 (`docs/architecture/server.md`, "Accounts and tokens"): with an
//! identity token from Orchester Cloud, or without the internet with a code. Either way the server issues the device
//! its own device token.

use orchester_core::id::{ApprovalId, ProjectId};
use serde::{Deserialize, Serialize};

use crate::{Credential, DeviceKind, InviteKind, MemberRole, Session, User, UserRef};

/// `POST /auth/cloud`.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CloudJoin {
  pub identity_token: Credential,
  /// The newest key set the app carries from the cloud, for a server that cannot reach the cloud itself.
  pub key_set: Option<String>,
  pub device: String,
  /// The join approval this request finishes, once it was approved.
  pub approval: Option<ApprovalId>,
  /// `admin_web` for a sign-in to the server admin, with a token issued for that purpose.
  #[serde(default)]
  pub kind: DeviceKind,
}

/// `POST /auth/code`. `username` and `display_name` only when the code creates an account.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CodeJoin {
  pub code: Credential,
  pub device: String,
  pub username: Option<String>,
  pub display_name: Option<String>,
  /// `admin_web` for a sign-in to the server admin: with a setup, pairing or account code.
  #[serde(default)]
  pub kind: DeviceKind,
  /// An app's Orchester Cloud identity token for this server (no purpose), whose `mfa` claim shows its person signed
  /// in with two-factor authentication: every code but the setup code needs it in the app.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub identity_token: Option<Credential>,
  /// The newest key set the app carries from the cloud, for a server that cannot reach the cloud itself.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub key_set: Option<String>,
}

/// `POST /auth/setup`: whether `code` is the server's open setup code, without using it up. The server admin asks
/// before it asks for the first admin's name.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SetupCheck {
  pub code: Credential,
}

/// The project a join made the person a member of.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct JoinedProject {
  pub project: ProjectId,
  pub role: MemberRole,
}

/// `201` of a join: the device token, shown once.
#[derive(Debug, Serialize, Deserialize)]
pub struct Joined {
  pub token: Credential,
  pub session: Session,
  pub user: User,
  pub joined: Option<JoinedProject>,
}

/// `POST /auth/cloud/accept`, from the team server's web app: an identity token of purpose `accept` naming an open
/// invite, which the browser brought from Orchester Cloud (`docs/architecture/server.md`, "Accepting outside
/// Orchester"). No device: an app's device comes only from an app's own join.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CloudAccept {
  pub identity_token: Credential,
  /// The newest key set the cloud's page sent with the token, for a server that cannot reach the cloud itself.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub key_set: Option<String>,
}

/// What accepting an invite gave: for a project invite its project and role; for a server invite neither (an admin
/// invite made the account an admin); for an account invite only the link to the cloud identity.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcceptedMembership {
  pub kind: InviteKind,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub project: Option<ProjectId>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub role: Option<MemberRole>,
}

/// A browser signed in to the team server's web app (the browser kind of device token, 12 hours), shown once.
#[derive(Debug, Serialize, Deserialize)]
pub struct BrowserSession {
  pub token: Credential,
  pub session: Session,
  pub user: User,
}

/// `201` of `POST /auth/cloud/accept`: the membership made and the receipt the server signed for the cloud
/// (`statements::InviteAcceptance`), with a browser sign-in unless `approval_required`: join approval is on and the
/// account existed already, so the person signs in with a code from one of their computers instead.
#[derive(Debug, Serialize, Deserialize)]
pub struct CloudAccepted {
  #[serde(default)]
  pub browser_session: Option<BrowserSession>,
  pub membership: AcceptedMembership,
  pub receipt: String,
  pub approval_required: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalState {
  Waiting,
  Approved,
  Refused,
}

/// A cloud join of an existing account while join approval is on: it waits until one of that account's devices or
/// an admin approves it.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Approval {
  pub id: ApprovalId,
  pub user: UserRef,
  pub device: String,
  /// The address the join came from.
  #[serde(default)]
  pub address: String,
  pub state: ApprovalState,
  pub requested_at: i64,
  pub expires_at: i64,
}

/// How a cloud join was answered.
#[derive(Debug)]
pub enum CloudJoinAnswer {
  Joined(Joined),
  /// Join approval holds the join until one of the account's devices or an admin approves it.
  Waiting(Approval),
}

/// `202` of `POST /auth/cloud` while the join waits for approval.
#[derive(Debug, Serialize, Deserialize)]
pub struct ApprovalResponse {
  pub approval: Approval,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Approvals {
  pub approvals: Vec<Approval>,
}

/// `POST /auth/approvals/{id}`: the person's answer to *Approve this computer?*.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApprovalDecision {
  pub approve: bool,
}

/// `201` of `POST /auth/pairings`: a code for the same account's next device, shown once.
#[derive(Debug, Serialize, Deserialize)]
pub struct PairingCode {
  pub code: Credential,
  pub expires_at: i64,
}
