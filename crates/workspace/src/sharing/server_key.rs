//! Whether a team server holds the key whose id it gives itself, on this very connection
//! (`docs/architecture/server.md`, "Codes"). A server's id is its public key's; before this computer asks Orchester
//! Cloud for a code's token for that id, the server signs a fresh nonce with that key, together with the fingerprint of
//! the certificate its connections present (`POST /server/key-proof`). It is compared with the certificate of the
//! connection that carried the proof, pinned or checked by the trust store; to a client that named that certificate,
//! a server behind a proxy also names the one the proxy presented before a renewal it just saw. A server that names
//! another's id cannot sign, and one that passes the nonce on to the other and returns its proof is caught by the
//! certificate, unless the two present the same one: servers that share a certificate cannot be told apart this way.
//! Over plain HTTP there is no certificate to compare, so no token is asked for.

use orchester_api::{
  client::Client,
  identity::{key_proof_nonce, verify_key_proof},
};
use orchester_core::id::TeamServerId;

use super::SharingError;

/// `Ok` once the server `client` reaches proved it holds the key whose id is `said`, naming the certificate of the
/// connection that carried the proof; [`SharingError::UnprovenServer`] otherwise. Nothing but the nonce is sent.
pub(super) async fn proven(client: &Client, said: TeamServerId) -> Result<(), SharingError> {
  let nonce = key_proof_nonce().map_err(|_| SharingError::UnprovenServer)?;
  let answer = client.key_proof(&nonce).await?;
  let certificate = client.certificate().ok_or(SharingError::UnprovenServer)?.to_hex();
  verify_key_proof(&answer, said, (&nonce, &certificate)).map_err(|_| {
    // The server's admin reads this when a proxy's new certificate has not reached a fixed `proxy_certificate`, or
    // when `proxy_certificate = "auto"` cannot read it.
    tracing::warn!("a team server's key proof did not name this connection's certificate, or did not verify");
    SharingError::UnprovenServer
  })
}
