//! A real in-memory vault with controlled delay/failures, confined to the native QA example.
use std::{
  collections::HashMap,
  sync::atomic::{AtomicBool, AtomicU8, Ordering},
  time::Duration,
};

use orchester_core::secret::{SecretValue, SecretVault, VaultError, VaultKey};
use parking_lot::{Condvar, Mutex};

#[derive(Default)]
pub struct QaVault {
  values: Mutex<HashMap<VaultKey, SecretValue>>,
  held: Mutex<bool>,
  released: Condvar,
  failure: AtomicU8,
  entered: AtomicBool,
}

impl QaVault {
  pub fn configure(&self, held: bool, failure: u8) {
    self.entered.store(false, Ordering::SeqCst);
    self.failure.store(failure, Ordering::SeqCst);
    *self.held.lock() = held;
    self.released.notify_all();
  }

  pub fn entered(&self) -> bool {
    self.entered.load(Ordering::SeqCst)
  }

  fn ready(&self) -> Result<(), VaultError> {
    self.entered.store(true, Ordering::SeqCst);
    let mut held = self.held.lock();
    while *held {
      if self.released.wait_for(&mut held, Duration::from_secs(60)).timed_out() && *held {
        return Err(VaultError::Failed("QA hold timed out".into()));
      }
    }
    match self.failure.load(Ordering::SeqCst) {
      1 => Err(VaultError::AccessDenied("QA denial".into())),
      2 => Err(VaultError::Failed("QA backend failure".into())),
      _ => Ok(()),
    }
  }
}

impl SecretVault for QaVault {
  fn store(&self, key: &VaultKey, value: &SecretValue) -> Result<(), VaultError> {
    self.ready()?;
    self.values.lock().insert(
      key.clone(),
      SecretValue::new(value.expose().to_owned()).expect("the supplied SecretValue is nonblank"),
    );
    Ok(())
  }

  fn read(&self, key: &VaultKey) -> Result<SecretValue, VaultError> {
    self
      .values
      .lock()
      .get(key)
      .map(|value| SecretValue::new(value.expose().to_owned()).expect("stored SecretValue is nonblank"))
      .ok_or(VaultError::NotFound)
  }

  fn delete(&self, key: &VaultKey) -> Result<(), VaultError> {
    self.ready()?;
    self.values.lock().remove(key);
    Ok(())
  }
}
