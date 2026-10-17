//! Whether a team server holds the key whose id it gives itself, on this very connection
//! (`docs/architecture/server.md`, "Codes"). A server's id is its public key's; before this computer asks Orchester
//! Cloud for a code's token for that id, the server signs a fresh nonce with that key, together with the fingerprint of
//! the certificate its connections present (`POST /server/key-proof`). A server that names another's id cannot sign,
//! and one that passes the nonce on to the other and returns its proof is caught by the certificate, which is not the
//! one this computer connected to, unless the two present the same certificate: servers that share one cannot be told
//! apart this way. Over plain HTTP there is no certificate to compare, so no token is asked for.

use orchester_api::{
  client::{Client, ServerAddress},
  identity::{key_proof_nonce, verify_key_proof},
};
use orchester_core::id::TeamServerId;

use super::{SharingError, trust::Trust};

/// `Ok` once the server `client` reaches at `address`, trusted as `trust`, proved it holds the key whose id is `said`
/// over a connection presenting the certificate this computer sees; [`SharingError::UnprovenServer`] otherwise.
/// Nothing but the nonce is sent.
pub(super) async fn proven(
  client: &Client,
  said: TeamServerId,
  (address, trust): (&ServerAddress, Trust),
) -> Result<(), SharingError> {
  let certificate = trust
    .certificate(address)
    .await
    .map_err(|error| trust.refusal(error))?
    .ok_or(SharingError::UnprovenServer)?;
  let nonce = key_proof_nonce().map_err(|_| SharingError::UnprovenServer)?;
  let answer = client.key_proof(&nonce).await?;
  verify_key_proof(&answer, said, (&nonce, &certificate)).map_err(|_| SharingError::UnprovenServer)
}
