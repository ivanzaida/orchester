//! An in-memory project database and keychain, each able to fail on request.

use std::collections::HashMap;

use orchester_core::{
  id::SecretId,
  secret::{Environment, NewSecret, Secret, SecretKind, SecretName, SecretValue, SecretVault, VaultError, VaultKey},
  store::SecretStore,
  time::Timestamp,
};
use parking_lot::Mutex;
use thiserror::Error;

/// A fake value; no test uses a real credential.
pub const VALUE: &str = "fake-value-9fK2";

pub fn value(text: &str) -> SecretValue {
  SecretValue::new(text.to_owned()).expect("not blank")
}

pub fn at(millis: i64) -> Timestamp {
  Timestamp::from_unix_millis(millis)
}

pub fn new_secret(name: &str, environment: Environment) -> NewSecret {
  NewSecret {
    name: SecretName::new(name).expect("valid name"),
    kind: SecretKind::DatabaseConnection,
    environment,
  }
}

#[derive(Debug, Error)]
#[error("the fake database failed")]
pub struct StoreFailed;

/// The secrets table in memory.
#[derive(Default)]
pub struct MemoryStore {
  pub rows: Vec<Secret>,
  /// The next write fails.
  pub fail_writes: bool,
}

impl SecretStore for MemoryStore {
  type Error = StoreFailed;

  fn create_secret(&mut self, secret: &Secret) -> Result<(), StoreFailed> {
    if self.fail_writes {
      return Err(StoreFailed);
    }
    self.rows.push(secret.clone());
    Ok(())
  }

  fn save_secret(&mut self, secret: &Secret) -> Result<(), StoreFailed> {
    if self.fail_writes {
      return Err(StoreFailed);
    }
    let row = self
      .rows
      .iter_mut()
      .find(|row| row.id() == secret.id())
      .ok_or(StoreFailed)?;
    *row = secret.clone();
    Ok(())
  }

  fn secret(&self, id: SecretId) -> Result<Option<Secret>, StoreFailed> {
    Ok(self.rows.iter().find(|row| row.id() == id).cloned())
  }

  fn secrets(&self) -> Result<Vec<Secret>, StoreFailed> {
    Ok(self.rows.iter().filter(|row| !row.is_archived()).cloned().collect())
  }
}

/// A keychain in memory. It keeps what it is given, so tests can check the value arrived.
#[derive(Default)]
pub struct MemoryVault {
  values: Mutex<HashMap<VaultKey, String>>,
  failure: Mutex<Option<VaultError>>,
}

impl MemoryVault {
  pub fn value(&self, key: &VaultKey) -> Option<String> {
    self.values.lock().get(key).cloned()
  }

  pub fn len(&self) -> usize {
    self.values.lock().len()
  }

  /// Every later request fails with `error`.
  pub fn fail_with(&self, error: VaultError) {
    *self.failure.lock() = Some(error);
  }

  fn check(&self) -> Result<(), VaultError> {
    self.failure.lock().clone().map_or(Ok(()), Err)
  }
}

impl SecretVault for MemoryVault {
  fn store(&self, key: &VaultKey, value: &SecretValue) -> Result<(), VaultError> {
    self.check()?;
    self.values.lock().insert(key.clone(), value.expose().to_owned());
    Ok(())
  }

  fn delete(&self, key: &VaultKey) -> Result<(), VaultError> {
    self.check()?;
    self.values.lock().remove(key);
    Ok(())
  }
}
