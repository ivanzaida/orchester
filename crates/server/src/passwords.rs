use std::{fmt, sync::Arc};

use argon2::{
  Algorithm, Argon2, Params, PasswordHasher, PasswordVerifier, Version,
  password_hash::{
    PasswordHash as ParsedHash, SaltString,
    rand_core::{OsRng, RngCore},
  },
};
use orchester_api::{Credential, ErrorCode};
use sha2::{Digest, Sha256};
use tokio::sync::Semaphore;

use crate::error::ServerError;

// Fixed PHC value with the same work parameters as real passwords; never authenticates a user.
const DUMMY_HASH: &str =
  "$argon2id$v=19$m=19456,t=2,p=1$b3JjaGVzdGVyLWR1bW15IQ$AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";

#[derive(Clone)]
pub(crate) struct PasswordHash(String);

impl PasswordHash {
  pub fn stored(value: String) -> Self {
    Self(value)
  }
  pub fn expose(&self) -> &str {
    &self.0
  }
  pub fn dummy() -> Self {
    Self(DUMMY_HASH.into())
  }
}

impl fmt::Debug for PasswordHash {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str("PasswordHash(<redacted>)")
  }
}

pub(crate) struct Passwords {
  slots: Arc<Semaphore>,
}

impl Passwords {
  pub fn new() -> Self {
    Self {
      slots: Arc::new(Semaphore::new(4)),
    }
  }

  pub async fn hash(&self, password: Credential) -> Result<PasswordHash, ServerError> {
    let length = password.expose().chars().count();
    if !(12..=256).contains(&length) {
      return Err(ErrorCode::Invalid.into());
    }
    let permit = self
      .slots
      .clone()
      .acquire_owned()
      .await
      .map_err(|_| ServerError::Worker)?;
    tokio::task::spawn_blocking(move || {
      let _permit = permit;
      let mut salt = [0u8; 16];
      OsRng.try_fill_bytes(&mut salt).map_err(|_| ServerError::Random)?;
      let salt = SaltString::encode_b64(&salt).map_err(|_| ServerError::PasswordHash)?;
      hasher()
        .hash_password(password.expose().as_bytes(), &salt)
        .map(|hash| PasswordHash(hash.to_string()))
        .map_err(|_| ServerError::PasswordHash)
    })
    .await
    .map_err(|_| ServerError::Worker)?
  }

  pub async fn verify(&self, password: Credential, hash: PasswordHash) -> Result<bool, ServerError> {
    let permit = self
      .slots
      .clone()
      .acquire_owned()
      .await
      .map_err(|_| ServerError::Worker)?;
    tokio::task::spawn_blocking(move || {
      let _permit = permit;
      let hash = ParsedHash::new(hash.expose()).map_err(|_| ServerError::StoredData)?;
      match hasher().verify_password(password.expose().as_bytes(), &hash) {
        Ok(()) => Ok(true),
        Err(argon2::password_hash::Error::Password) => Ok(false),
        Err(_) => Err(ServerError::PasswordHash),
      }
    })
    .await
    .map_err(|_| ServerError::Worker)?
  }
}

fn hasher() -> Argon2<'static> {
  let params = Params::new(19 * 1024, 2, 1, None).expect("OWASP Argon2id parameters are valid");
  Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
}

pub(crate) fn random_credential(prefix: &str, bytes: usize) -> Result<Credential, ServerError> {
  let mut random = vec![0u8; bytes];
  OsRng.try_fill_bytes(&mut random).map_err(|_| ServerError::Random)?;
  let mut value = String::with_capacity(prefix.len() + bytes * 2);
  value.push_str(prefix);
  const HEX: &[u8; 16] = b"0123456789abcdef";
  for byte in random {
    value.push(HEX[usize::from(byte >> 4)] as char);
    value.push(HEX[usize::from(byte & 15)] as char);
  }
  Ok(Credential::new(value))
}

pub(crate) fn token_hash(token: &Credential) -> Vec<u8> {
  Sha256::digest(token.expose().as_bytes()).to_vec()
}
