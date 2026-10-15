//! The server's own identity and keys, made once by the API 3 migration and kept in its database: its id (the
//! audience of identity tokens for it), its Ed25519 key for what it signs for the cloud, and the HMAC key of its codes.

use orchester_core::id::TeamServerId;
use ring::{
  hmac,
  rand::{SecureRandom, SystemRandom},
  signature::{Ed25519KeyPair, KeyPair},
};
use rusqlite::{Connection, params};
use serde::Serialize;

use crate::{cloud_link::jws, error::ServerError};

pub(crate) struct ServerKeys {
  pub id: TeamServerId,
  signing: Ed25519KeyPair,
  code_key: hmac::Key,
}

impl ServerKeys {
  /// Makes the server's id and keys; run once, by the migration that adds the table.
  pub fn create(db: &Connection) -> Result<(), ServerError> {
    let signing = Ed25519KeyPair::generate_pkcs8(&SystemRandom::new()).map_err(|_| ServerError::Random)?;
    let code_key = random::<32>()?;
    db.execute(
      "INSERT INTO server_keys(id,server_id,signing_key,code_key) VALUES (1,?1,?2,?3)",
      params![
        TeamServerId::generate().to_string(),
        signing.as_ref(),
        code_key.as_slice()
      ],
    )?;
    Ok(())
  }

  pub fn load(db: &Connection) -> Result<Self, ServerError> {
    let (id, signing, code_key): (String, Vec<u8>, Vec<u8>) = db.query_row(
      "SELECT server_id,signing_key,code_key FROM server_keys WHERE id=1",
      [],
      |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    )?;
    Ok(Self {
      id: id.parse().map_err(|_| ServerError::StoredData)?,
      signing: Ed25519KeyPair::from_pkcs8(&signing).map_err(|_| ServerError::StoredData)?,
      code_key: hmac::Key::new(hmac::HMAC_SHA256, &code_key),
    })
  }

  /// The server's public key in base64url, as the cloud records it.
  pub fn public_key(&self) -> String {
    jws::encode(self.signing.public_key().as_ref())
  }

  /// `payload` as a compact JWS of the type `typ` signed with the server's key.
  pub fn sign(&self, typ: &str, payload: &impl Serialize) -> String {
    jws::sign(&self.signing, typ, payload)
  }

  /// The HMAC of a code as stored; the code itself is never stored.
  pub fn code_tag(&self, code: &str) -> Vec<u8> {
    hmac::sign(&self.code_key, code.as_bytes()).as_ref().to_vec()
  }

  /// Whether `code` has the stored HMAC `tag`, compared in constant time.
  pub fn code_matches(&self, code: &str, tag: &[u8]) -> bool {
    hmac::verify(&self.code_key, code.as_bytes(), tag).is_ok()
  }
}

/// Random bytes from the operating system.
pub(crate) fn random<const N: usize>() -> Result<[u8; N], ServerError> {
  let mut bytes = [0_u8; N];
  SystemRandom::new().fill(&mut bytes).map_err(|_| ServerError::Random)?;
  Ok(bytes)
}
