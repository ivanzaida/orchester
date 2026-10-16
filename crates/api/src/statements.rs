//! What a team server signs with its own key for Orchester Cloud (`docs/architecture/server.md`, "Linking a team
//! server at setup", "Invites", "Codes"): its link statement, its invites and their revocations, and for an app its
//! proof that it holds the key of the id it gives itself, each a compact JWS of its own type. The cloud verifies them
//! with the server's public key; the server admin's page may carry them there.

use orchester_core::id::{InviteId, TeamServerId};
use serde::{Deserialize, Serialize};

use crate::{InviteKind, MemberRole};

pub const LINK_STATEMENT_TYPE: &str = "orchester-link+jws";
pub const SIGNED_INVITE_TYPE: &str = "orchester-invite+jws";
pub const INVITE_REVOCATION_TYPE: &str = "orchester-invite-revocation+jws";
pub const KEY_PROOF_TYPE: &str = "orchester-key-proof+jws";

/// What the server signs to be linked: its id (its public key's, `identity::PublicKey::server_id`), name, address,
/// public key, and the SHA-256 of its own certificate in hex (which apps pin; `None` behind a proxy's certificate).
/// `issued_at` in seconds since the Unix epoch; `nonce` (random, base64url) makes each statement usable once.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LinkStatement {
  pub server: TeamServerId,
  pub nonce: String,
  pub name: String,
  pub address: String,
  pub public_key: String,
  pub fingerprint: Option<String>,
  pub issued_at: i64,
}

/// A project an invite is for, as the invitee reads it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InvitedProject {
  pub address: String,
  pub name: String,
}

/// An invite by email for the cloud to route. Times in milliseconds since the Unix epoch.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignedInvite {
  pub id: InviteId,
  pub server: TeamServerId,
  pub kind: InviteKind,
  pub email: String,
  pub project: Option<InvitedProject>,
  pub role: Option<MemberRole>,
  pub inviter: String,
  pub expires_at: i64,
  pub issued_at: i64,
}

/// The server revoked an invite by email; the cloud deletes it. `issued_at` in milliseconds.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InviteRevocation {
  pub id: InviteId,
  pub server: TeamServerId,
  pub issued_at: i64,
}

/// The server's answer to an app's `nonce` (`POST /server/key-proof`): signed with the key whose id is `server`, so an
/// app that checks the signature with that key, and the key's id against `server`, knows it talks to the holder of the
/// id it asks Orchester Cloud for a code's token for. `issued_at` in seconds.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KeyProof {
  pub server: TeamServerId,
  pub nonce: String,
  pub issued_at: i64,
}
