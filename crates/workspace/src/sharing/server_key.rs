//! Whether a team server holds the key whose id it gives itself (`docs/architecture/server.md`, "Codes"). A server's
//! id is its public key's; before this computer asks Orchester Cloud for a code's token for that id, the server signs
//! a fresh nonce with that key (`POST /server/key-proof`). A server that names another's id cannot, so it never gets a
//! token it could pass on as that server's, whatever certificate or proxy it is reached through.

use orchester_api::{
  client::Client,
  identity::{key_proof_nonce, verify_key_proof},
};
use orchester_core::id::TeamServerId;

use super::SharingError;

/// `Ok` once the server `client` reaches proved it holds the key whose id is `said`; [`SharingError::UnprovenServer`]
/// otherwise. Nothing but the nonce is sent.
pub(super) async fn proven(client: &Client, said: TeamServerId) -> Result<(), SharingError> {
  let nonce = key_proof_nonce().map_err(|_| SharingError::UnprovenServer)?;
  let answer = client.key_proof(&nonce).await?;
  verify_key_proof(&answer, said, &nonce).map_err(|_| SharingError::UnprovenServer)
}
