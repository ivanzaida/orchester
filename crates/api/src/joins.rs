//! How a device joins a team server in API 3 (`docs/architecture/server.md`, "Accounts and tokens"): with an
//! identity token from Orchester Cloud, or without the internet with a code. Either way the server issues the device
//! its own device token.

use orchester_core::id::{ApprovalId, ProjectId};
use serde::{Deserialize, Serialize};

use crate::{Credential, MemberRole, Session, User, UserRef};

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
}

/// `POST /auth/code`. `username` and `display_name` only when the code creates an account.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CodeJoin {
  pub code: Credential,
  pub device: String,
  pub username: Option<String>,
  pub display_name: Option<String>,
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
  pub state: ApprovalState,
  pub requested_at: i64,
  pub expires_at: i64,
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
