//! Project secrets on the desktop: the keychain the session keeps values in, and reading, adding,
//! replacing and archiving secrets.

pub mod data;
pub mod form;
mod replacement;

use std::sync::Arc;

use orchester_core::secret::SecretVault;
use orchester_vault::KeychainVault;

/// The keychain secret values go to, shared with screens through the UI context. The application
/// uses the OS keychain; tests hand in one in memory, so screens can be driven without touching
/// the keychain of the user running them.
#[derive(Clone)]
pub struct Vault(Arc<dyn SecretVault>);

impl Vault {
  /// The keychain of the user signed in to this computer.
  pub fn keychain() -> Self {
    Self(Arc::new(KeychainVault))
  }

  /// Values kept by `vault`, e.g. one in memory in tests.
  pub fn with(vault: Arc<dyn SecretVault>) -> Self {
    Self(vault)
  }

  pub fn get(&self) -> &dyn SecretVault {
    &*self.0
  }
}

impl PartialEq for Vault {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.0, &other.0)
  }
}
