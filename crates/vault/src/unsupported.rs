//! Platforms other than Windows and macOS have no keychain store here yet. Every request fails
//! with [`VaultError::Unsupported`], which the Secrets screen shows, instead of storing a value
//! somewhere less safe.

use orchester_core::secret::{SecretValue, VaultError, VaultKey};

pub(crate) fn store(_key: &VaultKey, _value: &SecretValue) -> Result<(), VaultError> {
  Err(VaultError::Unsupported)
}

pub(crate) fn read(_key: &VaultKey) -> Result<SecretValue, VaultError> {
  Err(VaultError::Unsupported)
}

pub(crate) fn delete(_key: &VaultKey) -> Result<(), VaultError> {
  Err(VaultError::Unsupported)
}
