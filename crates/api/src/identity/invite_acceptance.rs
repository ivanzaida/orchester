//! A team server's receipt for an invite accepted in the browser (`docs/architecture/server.md`, "Accepting outside
//! Orchester"): the server signs it with its own key when `POST /auth/cloud/accept` made the membership, and the cloud
//! checks it with the key the server linked with before it records the join.

use orchester_core::id::{InviteId, TeamServerId};

use super::{IdentityError, PublicKey, SigningKey, verify_statement};
use crate::statements::{INVITE_ACCEPTANCE_TYPE, InviteAcceptance};

impl SigningKey {
  /// `receipt` as the compact JWS of type `orchester-invite-accepted+jws` that the cloud's `POST
  /// /invites/{id}/accepted` takes.
  pub fn sign_invite_acceptance(&self, receipt: &InviteAcceptance) -> Result<String, IdentityError> {
    self.sign_statement(INVITE_ACCEPTANCE_TYPE, receipt)
  }
}

/// The receipt `compact` if `key`, the key the server `server` linked with, signed it for the invite `invite` of that
/// server. `WrongAudience` for a receipt of another invite or server, which the cloud must not record for this one.
pub fn verify_invite_acceptance(
  compact: &str,
  key: &PublicKey,
  (invite, server): (InviteId, TeamServerId),
) -> Result<InviteAcceptance, IdentityError> {
  let receipt: InviteAcceptance = verify_statement(compact, INVITE_ACCEPTANCE_TYPE, key)?;
  if receipt.id != invite || receipt.server != server {
    return Err(IdentityError::WrongAudience);
  }
  Ok(receipt)
}
