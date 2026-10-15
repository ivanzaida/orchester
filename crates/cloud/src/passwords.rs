//! Argon2id with S1a's parameters and rules, moved from the team server (`docs/architecture/server.md`, rows 46 and
//! 48): 12 to 256 characters, no composition rules, hashing on a bounded pool.

use std::{fmt, sync::Arc, time::Duration};

use argon2::{
  Algorithm, Argon2, Params, PasswordHasher, PasswordVerifier, Version,
  password_hash::{PasswordHash as ParsedHash, SaltString},
};
use orchester_api::{Credential, cloud::CloudErrorCode};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

use crate::{clock::Clock, error::CloudError, limits::Attempt, tokens::random_bytes};

// A fixed PHC value with the same work parameters as real passwords; it never authenticates anyone.
const DUMMY_HASH: &str =
  "$argon2id$v=19$m=19456,t=2,p=1$b3JjaGVzdGVyLWR1bW15IQ$AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";

/// An argon2id PHC string; `Debug` never shows it.
#[derive(Clone, PartialEq, Eq)]
pub struct PasswordHash(String);

impl PasswordHash {
  pub(crate) fn stored(value: String) -> Self {
    Self(value)
  }

  pub(crate) fn expose(&self) -> &str {
    &self.0
  }

  /// What an unknown email is verified against, so it answers like a wrong password and takes as long.
  pub(crate) fn dummy() -> Self {
    Self(DUMMY_HASH.into())
  }
}

impl fmt::Debug for PasswordHash {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str("PasswordHash(<redacted>)")
  }
}

/// Hashing runs on `slots` blocking workers at a time; a request that waits longer than `wait` for one is refused
/// with `503 busy` instead of queueing without bound (review 1, M6).
pub struct Passwords {
  slots: Arc<Semaphore>,
  wait: Duration,
}

impl Passwords {
  pub fn new(slots: usize, wait: Duration) -> Self {
    Self {
      slots: Arc::new(Semaphore::new(slots)),
      wait,
    }
  }

  async fn slot(&self) -> Result<OwnedSemaphorePermit, CloudError> {
    tokio::time::timeout(self.wait, self.slots.clone().acquire_owned())
      .await
      .map_err(|_| CloudError::from(CloudErrorCode::Busy))?
      .map_err(|_| CloudError::Worker)
  }

  pub fn check(password: &Credential) -> Result<(), CloudError> {
    let length = password.expose().chars().count();
    if (12..=256).contains(&length) {
      Ok(())
    } else {
      Err(CloudErrorCode::Invalid.into())
    }
  }

  pub async fn hash(&self, password: Credential) -> Result<PasswordHash, CloudError> {
    Self::check(&password)?;
    let permit = self.slot().await?;
    let salt = random_bytes::<16>()?;
    tokio::task::spawn_blocking(move || {
      let _permit = permit;
      let salt = SaltString::encode_b64(&salt).map_err(|_| CloudError::PasswordHash)?;
      hasher()
        .hash_password(password.expose().as_bytes(), &salt)
        .map(|hash| PasswordHash(hash.to_string()))
        .map_err(|_| CloudError::PasswordHash)
    })
    .await
    .map_err(|_| CloudError::Worker)?
  }

  pub(crate) async fn verify(
    &self,
    password: Credential,
    hash: PasswordHash,
    mut attempt: Attempt,
    clock: &dyn Clock,
  ) -> Result<(bool, Attempt), CloudError> {
    let permit = self.slot().await?;
    attempt.start(clock.now_ms())?;
    let verified = tokio::task::spawn_blocking(move || {
      let _permit = permit;
      matches(&password, &hash)
    })
    .await
    .map_err(|_| CloudError::Worker)??;
    Ok((verified, attempt))
  }

  /// Whether `password` is the one `hash` was made from; `None` is checked against the dummy hash, so a missing
  /// hash takes as long as a wrong password.
  pub(crate) async fn matches(&self, password: Credential, hash: Option<PasswordHash>) -> Result<bool, CloudError> {
    let permit = self.slot().await?;
    let known = hash.is_some();
    let hash = hash.unwrap_or_else(PasswordHash::dummy);
    let verified = tokio::task::spawn_blocking(move || {
      let _permit = permit;
      matches(&password, &hash)
    })
    .await
    .map_err(|_| CloudError::Worker)??;
    Ok(verified && known)
  }
}

fn matches(password: &Credential, hash: &PasswordHash) -> Result<bool, CloudError> {
  let hash = ParsedHash::new(hash.expose()).map_err(|_| CloudError::StoredData)?;
  match hasher().verify_password(password.expose().as_bytes(), &hash) {
    Ok(()) => Ok(true),
    Err(argon2::password_hash::Error::Password) => Ok(false),
    Err(_) => Err(CloudError::PasswordHash),
  }
}

fn hasher() -> Argon2<'static> {
  let params = Params::new(19 * 1024, 2, 1, None).expect("OWASP Argon2id parameters are valid");
  Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
}
