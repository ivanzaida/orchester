//! The team servers Orchester Cloud knows and the invites in flight (`docs/architecture/server.md`, "Orchester Cloud",
//! "Invites"). A team server signs its link statement and each invite with its own key; the cloud registers the
//! server from the statement its owner carries, and accepts an invite that the registered key signed, whoever carries
//! it (the server itself, or the inviter's browser).

use orchester_core::id::{InviteId, TeamServerId};
use serde::{Deserialize, Serialize};

use super::CloudServer;
use crate::{Credential, InviteKind, MemberRole};

/// The JWS type of a team server's link statement.
pub const LINK_STATEMENT_TYPE: &str = "orchester-link+jws";
/// The JWS type of an invite a team server signed.
pub const INVITE_STATEMENT_TYPE: &str = "orchester-invite+jws";

/// What a team server signs at setup for the cloud to register it: its id, name, address and public key, and the
/// SHA-256 of its own certificate in hex (`None` with a CA's certificate). `issued_at` in seconds.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LinkStatement {
  pub server: TeamServerId,
  pub name: String,
  pub address: String,
  pub public_key: String,
  pub fingerprint: Option<String>,
  pub issued_at: i64,
}

/// A project an invite names: its address on the team server and its name.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InvitedProject {
  pub address: String,
  pub name: String,
}

/// An invite as its team server signs it for the cloud to route. `role` for a project invite, `admin` for a server
/// invite that makes the new account an admin. Times in milliseconds, as the team server keeps them.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InviteStatement {
  pub id: InviteId,
  pub server: TeamServerId,
  pub kind: InviteKind,
  pub email: String,
  pub project: Option<InvitedProject>,
  pub role: Option<MemberRole>,
  #[serde(default)]
  pub admin: bool,
  pub inviter: String,
  pub expires_at: i64,
  pub issued_at: i64,
}

/// `POST /servers`: the link statement the team server signed.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServerLink {
  pub link: String,
}

/// A team server of the account: one it owns (linked by it) or joined (by accepting an invite). `joined_at` is when
/// it was linked, for the owner, or first joined.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountServer {
  #[serde(flatten)]
  pub server: CloudServer,
  pub owner: bool,
  pub joined_at: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ServerResponse {
  pub server: AccountServer,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ServersResponse {
  pub servers: Vec<AccountServer>,
}

/// `POST /invites`: an invite its team server signed.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignedInvite {
  pub invite: String,
}

/// An open invite to the account's email.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CloudInvite {
  pub id: InviteId,
  pub kind: InviteKind,
  pub role: Option<MemberRole>,
  pub admin: bool,
  pub server: CloudServer,
  pub project: Option<InvitedProject>,
  pub inviter_display_name: String,
  pub expires_at: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InvitesResponse {
  pub invites: Vec<CloudInvite>,
}

/// `201` of `POST /invites/{id}/accept`: where to join, and a five-minute identity token for that server that names
/// the invite. The app connects to `server.address` trusting `server.fingerprint` before it hands the token over.
#[derive(Debug, Serialize, Deserialize)]
pub struct InviteAccepted {
  pub server: CloudServer,
  pub project: Option<InvitedProject>,
  pub identity_token: Credential,
  /// Seconds since the Unix epoch, as the token's own `exp`.
  pub expires_at: i64,
}

/// `DELETE /me`: the account's password, asked again before everything the cloud holds about it is deleted.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AccountDeletion {
  pub password: Credential,
}
