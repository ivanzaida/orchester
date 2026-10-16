//! A team server's proof that it holds the key whose id it gives itself (`POST /server/key-proof`,
//! `docs/architecture/server.md`, "Codes"): an app sends a fresh nonce, the server signs it with its key, and the app
//! checks that the key's id is the server's and that the signature covers that id and that nonce. Only then does the
//! app ask Orchester Cloud for a code's token for that id, so no server can pass one on as another's.

use orchester_core::id::TeamServerId;
use ring::rand::{SecureRandom, SystemRandom};

use super::{IdentityError, PublicKey, jws, verify_statement};
use crate::{
  KeyProofResponse,
  statements::{KEY_PROOF_TYPE, KeyProof},
};

/// A fresh nonce for one key proof: 16 random bytes in base64url.
pub fn key_proof_nonce() -> Result<String, IdentityError> {
  let mut bytes = [0_u8; 16];
  SystemRandom::new()
    .fill(&mut bytes)
    .map_err(|_| IdentityError::Random)?;
  Ok(jws::encode(&bytes))
}

/// Whether `answer` proves that the server answering holds the key whose id is `server`: the key's id is `server`,
/// and that key signed a key proof naming `server` and `nonce`. `WrongAudience` for a key or proof of another server.
pub fn verify_key_proof(answer: &KeyProofResponse, server: TeamServerId, nonce: &str) -> Result<(), IdentityError> {
  let key = PublicKey::parse(&answer.public_key)?;
  if key.server_id() != server {
    return Err(IdentityError::WrongAudience);
  }
  let proof: KeyProof = verify_statement(&answer.proof, KEY_PROOF_TYPE, &key)?;
  if proof.server != server || proof.nonce != nonce {
    return Err(IdentityError::WrongAudience);
  }
  Ok(())
}
