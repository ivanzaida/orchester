//! Device tokens (`docs/architecture/server.md`, "Device tokens"): 32 random bytes sent as `orc_` and 64 hex
//! characters, stored only as their SHA-256, one per device, never shown to anyone but the device it is issued to.

use orchester_api::{Credential, DeviceKind, ErrorCode, JoinedVia, Session, User};
use orchester_core::id::SessionId;
use rusqlite::Connection;
use sha2::{Digest, Sha256};

use crate::{
  db::{self, NewSession},
  error::ServerError,
  server_keys::random,
};

/// How a new device token came to be: the joining device's name and kind, how it joined, and the cloud signing key of
/// the identity token it was issued for and whether that token's device signed in with two-factor authentication.
pub(crate) struct Issued<'a> {
  pub device: &'a str,
  pub kind: DeviceKind,
  pub via: JoinedVia,
  pub kid: Option<&'a str>,
  pub mfa: bool,
}

/// A new device token for `user`, recorded in the caller's transaction.
pub(crate) fn issue(
  db: &Connection,
  user: &User,
  issued: &Issued<'_>,
  now: i64,
) -> Result<(Credential, Session), ServerError> {
  let Issued {
    device,
    kind,
    via,
    kid,
    mfa,
  } = *issued;
  let token = new_token()?;
  let session = Session {
    id: SessionId::generate(),
    device: device.to_owned(),
    created_at: now,
    last_used_at: now,
    current: true,
    via,
    mfa,
  };
  db::insert_session(
    db,
    &NewSession {
      id: session.id,
      user: user.id,
      hash: &token_hash(&token),
      device,
      via,
      kid,
      kind,
      mfa,
    },
    now,
  )?;
  Ok((token, session))
}

/// A device's name as a join gives it: not blank, at most 256 characters.
pub(crate) fn check_device(device: &str) -> Result<(), ServerError> {
  if device.trim().is_empty() || device.chars().count() > 256 {
    return Err(ErrorCode::Invalid.into());
  }
  Ok(())
}

pub(crate) fn token_hash(token: &Credential) -> Vec<u8> {
  Sha256::digest(token.expose().as_bytes()).to_vec()
}

fn new_token() -> Result<Credential, ServerError> {
  let bytes = random::<32>()?;
  let mut value = String::with_capacity(68);
  value.push_str("orc_");
  for byte in bytes {
    value.push_str(&format!("{byte:02x}"));
  }
  Ok(Credential::new(value))
}
