//! The harness's secret store, in memory: the token it makes is thrown away with the process, and reads can be
//! refused to show a token that cannot be read.

use std::{
  collections::HashMap,
  sync::atomic::{AtomicBool, Ordering},
};

use orchester_core::secret::{SecretValue, SecretVault, VaultError, VaultKey};
use parking_lot::Mutex;

#[derive(Default)]
pub struct MemoryVault {
  values: Mutex<HashMap<VaultKey, String>>,
  refuse_reads: AtomicBool,
}

impl MemoryVault {
  pub fn refuse_reads(&self, refuse: bool) {
    self.refuse_reads.store(refuse, Ordering::SeqCst);
  }
}

impl SecretVault for MemoryVault {
  fn store(&self, key: &VaultKey, value: &SecretValue) -> Result<(), VaultError> {
    self.values.lock().insert(key.clone(), value.expose().to_owned());
    Ok(())
  }

  fn read(&self, key: &VaultKey) -> Result<SecretValue, VaultError> {
    if self.refuse_reads.load(Ordering::SeqCst) {
      return Err(VaultError::AccessDenied("the QA harness refuses reads".to_owned()));
    }
    let value = self.values.lock().get(key).cloned().ok_or(VaultError::NotFound)?;
    Ok(SecretValue::new(value).expect("a stored token is never blank"))
  }

  fn delete(&self, key: &VaultKey) -> Result<(), VaultError> {
    self.values.lock().remove(key);
    Ok(())
  }
}
