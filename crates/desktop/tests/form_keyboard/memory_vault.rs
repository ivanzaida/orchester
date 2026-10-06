//! The keychain of the headless desktop: secret values go to memory, never to the keychain of the
//! user running the tests.

use std::collections::HashMap;

use orchester_core::secret::{SecretValue, SecretVault, VaultError, VaultKey};
use parking_lot::Mutex;

/// A keychain in memory. It keeps what it is given, so tests can check the value arrived.
#[derive(Default)]
pub struct MemoryVault(Mutex<HashMap<VaultKey, String>>);

impl MemoryVault {
  /// The stored values, in no particular order.
  pub fn values(&self) -> Vec<String> {
    self.0.lock().values().cloned().collect()
  }
}

impl SecretVault for MemoryVault {
  fn store(&self, key: &VaultKey, value: &SecretValue) -> Result<(), VaultError> {
    self.0.lock().insert(key.clone(), value.expose().to_owned());
    Ok(())
  }

  fn read(&self, key: &VaultKey) -> Result<SecretValue, VaultError> {
    let value = self.0.lock().get(key).cloned().ok_or(VaultError::NotFound)?;
    SecretValue::new(value).map_err(|_| VaultError::Failed("blank".to_owned()))
  }

  fn delete(&self, key: &VaultKey) -> Result<(), VaultError> {
    self.0.lock().remove(key);
    Ok(())
  }
}
