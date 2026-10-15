//! What a team server signs with its own key for the cloud: its link statement and its invites, each a compact JWS of
//! a named type (`docs/architecture/server.md`, "Identity tokens and keys", "Invites"). The cloud verifies an invite
//! with the key it registered for the server, and a link statement with the key the statement names, which the
//! server's owner carries to the cloud while signed in.

use serde::de::DeserializeOwned;

use super::{IdentityError, PublicKey, jws};

/// The statement's payload, its signature not checked: only for reading the key a link statement names, which must
/// then verify it with `verify_statement`.
pub fn unverified_statement<T: DeserializeOwned>(compact: &str, typ: &str) -> Result<T, IdentityError> {
  jws::parse(&jws::decode(compact, typ)?.payload)
}

/// The statement's payload once `key` verifies its signature.
pub fn verify_statement<T: DeserializeOwned>(compact: &str, typ: &str, key: &PublicKey) -> Result<T, IdentityError> {
  let compact = jws::decode(compact, typ)?;
  key.verify(&compact.signing_input, &compact.signature)?;
  jws::parse(&compact.payload)
}
