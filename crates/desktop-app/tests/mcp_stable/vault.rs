//! A secret store in memory that can refuse reads or stores, or hold reads until released, so the person's own
//! keychain is never touched and races with the store can be staged.

use std::{
  collections::HashMap,
  sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};

use orchester_core::secret::{SecretValue, SecretVault, VaultError, VaultKey};
use parking_lot::{Condvar, Mutex};

#[derive(Default)]
pub struct MemoryVault {
  values: Mutex<HashMap<VaultKey, String>>,
  refuse_reads: AtomicBool,
  refuse_stores: AtomicBool,
  /// Whether reads wait until [`MemoryVault::release_reads`].
  held: Mutex<bool>,
  released: Condvar,
  /// Reads waiting now.
  waiting: AtomicUsize,
  stores: AtomicUsize,
}

#[allow(dead_code, reason = "the Settings tests share this store and do not stage races")]
impl MemoryVault {
  pub fn refuse_reads(&self, refuse: bool) {
    self.refuse_reads.store(refuse, Ordering::SeqCst);
  }

  pub fn refuse_stores(&self, refuse: bool) {
    self.refuse_stores.store(refuse, Ordering::SeqCst);
  }

  /// The values kept, by key.
  pub fn values(&self) -> HashMap<VaultKey, String> {
    self.values.lock().clone()
  }

  /// Reads from now on wait until [`MemoryVault::release_reads`].
  pub fn hold_reads(&self) {
    *self.held.lock() = true;
  }

  pub fn release_reads(&self) {
    *self.held.lock() = false;
    self.released.notify_all();
  }

  /// How many reads wait now.
  pub fn waiting_reads(&self) -> usize {
    self.waiting.load(Ordering::SeqCst)
  }

  /// How many values were stored.
  pub fn stores(&self) -> usize {
    self.stores.load(Ordering::SeqCst)
  }
}

impl SecretVault for MemoryVault {
  fn store(&self, key: &VaultKey, value: &SecretValue) -> Result<(), VaultError> {
    if self.refuse_stores.load(Ordering::SeqCst) {
      return Err(VaultError::AccessDenied("fixture denial".to_owned()));
    }
    self.stores.fetch_add(1, Ordering::SeqCst);
    self.values.lock().insert(key.clone(), value.expose().to_owned());
    Ok(())
  }

  fn read(&self, key: &VaultKey) -> Result<SecretValue, VaultError> {
    let mut held = self.held.lock();
    if *held {
      self.waiting.fetch_add(1, Ordering::SeqCst);
      while *held {
        self.released.wait(&mut held);
      }
      self.waiting.fetch_sub(1, Ordering::SeqCst);
    }
    drop(held);
    if self.refuse_reads.load(Ordering::SeqCst) {
      return Err(VaultError::AccessDenied("fixture denial".to_owned()));
    }
    let value = self.values.lock().get(key).cloned().ok_or(VaultError::NotFound)?;
    Ok(SecretValue::new(value).expect("a stored token is never blank"))
  }

  fn delete(&self, key: &VaultKey) -> Result<(), VaultError> {
    self.values.lock().remove(key);
    Ok(())
  }
}
