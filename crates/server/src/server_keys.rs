//! The server's own identity and keys, made once by the API 3 migration and kept in its database: its id (the
//! audience of identity tokens for it), its Ed25519 key for what it signs for the cloud, and the HMAC key of its codes.

use orchester_api::identity::{PrivateKey, SigningKey};
use orchester_core::id::TeamServerId;
use ring::{
  hmac,
  rand::{SecureRandom, SystemRandom},
};
use rusqlite::{Connection, params};
use serde::Serialize;

use crate::error::ServerError;

pub(crate) struct ServerKeys {
  pub id: TeamServerId,
  signing: SigningKey,
  code_key: hmac::Key,
}

impl ServerKeys {
  /// Makes the server's keys and its id, which is its signing key's (`PublicKey::server_id`); run once, by the
  /// migration that adds the table.
  pub fn create(db: &Connection) -> Result<(), ServerError> {
    let signing = PrivateKey::generate().map_err(|_| ServerError::Random)?;
    let code_key = random::<32>()?;
    let id = SigningKey::new(&signing)
      .map_err(|_| ServerError::Random)?
      .public_key()
      .server_id();
    db.execute(
      "INSERT INTO server_keys(id,server_id,signing_key,code_key) VALUES (1,?1,?2,?3)",
      params![id.to_string(), signing.pkcs8(), code_key.as_slice()],
    )?;
    Ok(())
  }

  /// Gives a server whose stored id is not its key's its key's id, and says so in the log: S1c's random ids, and
  /// schema 4 databases of earlier server admin builds (security review 2, 3). Safe under security review 1's B1:
  /// the key is this server's own, so the id derived from it can be no other server's. Run at every open.
  pub fn own_id(db: &Connection) -> Result<(), ServerError> {
    let (stored, signing): (String, Vec<u8>) =
      db.query_row("SELECT server_id,signing_key FROM server_keys WHERE id=1", [], |row| {
        Ok((row.get(0)?, row.get(1)?))
      })?;
    let id = SigningKey::new(&PrivateKey::from_pkcs8(signing))
      .map_err(|_| ServerError::StoredData)?
      .public_key()
      .server_id();
    if stored != id.to_string() {
      db.execute("UPDATE server_keys SET server_id=?1 WHERE id=1", [id.to_string()])?;
      tracing::warn!(
        from = %stored,
        to = %id,
        "the server's id is now its key's; if it was linked to Orchester Cloud, link it again in the admin's Settings"
      );
    }
    Ok(())
  }

  pub fn load(db: &Connection) -> Result<Self, ServerError> {
    let (id, signing, code_key): (String, Vec<u8>, Vec<u8>) = db.query_row(
      "SELECT server_id,signing_key,code_key FROM server_keys WHERE id=1",
      [],
      |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    )?;
    let signing = SigningKey::new(&PrivateKey::from_pkcs8(signing)).map_err(|_| ServerError::StoredData)?;
    let id: TeamServerId = id.parse().map_err(|_| ServerError::StoredData)?;
    // The id is the key's: a stored id that is not would let tokens for another server's id in.
    if id != signing.public_key().server_id() {
      return Err(ServerError::StoredData);
    }
    Ok(Self {
      id,
      signing,
      code_key: hmac::Key::new(hmac::HMAC_SHA256, &code_key),
    })
  }

  /// The server's public key in base64url, as the cloud records it.
  pub fn public_key(&self) -> String {
    self.signing.public_key().encoded()
  }

  /// `payload` as a compact JWS of the type `typ` signed with the server's key.
  pub fn sign(&self, typ: &str, payload: &impl Serialize) -> Result<String, ServerError> {
    self
      .signing
      .sign_statement(typ, payload)
      .map_err(|_| ServerError::StoredData)
  }

  /// The HMAC of a code as stored; the code itself is never stored.
  pub fn code_tag(&self, code: &str) -> Vec<u8> {
    hmac::sign(&self.code_key, code.as_bytes()).as_ref().to_vec()
  }

  /// Whether `code` has the stored HMAC `tag`, compared in constant time.
  pub fn code_matches(&self, code: &str, tag: &[u8]) -> bool {
    hmac::verify(&self.code_key, code.as_bytes(), tag).is_ok()
  }

  /// The HMAC an account keeps of its verified email (lowercased), never the email itself. The prefix keeps it apart
  /// from a code's: no code holds a NUL.
  pub fn email_tag(&self, email: &str) -> Vec<u8> {
    let message = format!("email\0{}", email.to_ascii_lowercase());
    hmac::sign(&self.code_key, message.as_bytes()).as_ref().to_vec()
  }
}

/// Random bytes from the operating system.
pub(crate) fn random<const N: usize>() -> Result<[u8; N], ServerError> {
  let mut bytes = [0_u8; N];
  SystemRandom::new().fill(&mut bytes).map_err(|_| ServerError::Random)?;
  Ok(bytes)
}
