//! A keychain in memory, for the runner's tests: the secrets MCP servers use.

use std::{collections::HashMap, sync::Arc};

use orchester_core::secret::{SecretValue, SecretVault, VaultError, VaultKey};
use parking_lot::Mutex;

#[derive(Default)]
pub struct MemoryVault(Mutex<HashMap<VaultKey, String>>);

impl MemoryVault {
  pub fn with(values: Vec<(VaultKey, &str)>) -> Arc<Self> {
    let vault = Self::default();
    vault
      .0
      .lock()
      .extend(values.into_iter().map(|(key, value)| (key, value.to_owned())));
    Arc::new(vault)
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
