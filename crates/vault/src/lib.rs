//! The OS keychain behind core's [`SecretVault`]: Windows Credential Manager and the macOS login
//! keychain, through the keyring crates. A crate of its own because the keychain is a boundary
//! both the desktop (which writes values) and the runner (which will read them at tool-call time)
//! reach, and it keeps the platform dependencies out of the other crates. Platforms without a
//! keychain here yet answer every request with [`VaultError::Unsupported`].

#[cfg(any(windows, target_os = "macos"))]
mod entry;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(not(any(windows, target_os = "macos")))]
mod unsupported;
#[cfg(windows)]
mod windows;

#[cfg(target_os = "macos")]
use macos as platform;
use orchester_core::secret::{SecretValue, SecretVault, VaultError, VaultKey};
#[cfg(not(any(windows, target_os = "macos")))]
use unsupported as platform;
#[cfg(windows)]
use windows as platform;

/// The keychain of the user signed in to this computer.
#[derive(Clone, Copy, Debug, Default)]
pub struct KeychainVault;

impl SecretVault for KeychainVault {
  fn store(&self, key: &VaultKey, value: &SecretValue) -> Result<(), VaultError> {
    platform::store(key, value)
  }

  fn delete(&self, key: &VaultKey) -> Result<(), VaultError> {
    platform::delete(key)
  }
}
