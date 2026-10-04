//! The macOS login keychain: one generic password per secret, with the service
//! `orchester:<project id>` and the account `<secret id>` (a shared server's token: the service
//! `orchester-server:<server id>` and the account `token`). Keychain Access lists it under the
//! service. Values are stored as their UTF-8 bytes.

use apple_native_keyring_store::keychain::Store;
use keyring_core::{Entry, api::CredentialStoreApi};
use orchester_core::secret::{SecretValue, VaultError, VaultKey};

use crate::entry;

pub(crate) fn store(key: &VaultKey, value: &SecretValue) -> Result<(), VaultError> {
  entry::store(credential(key), value)
}

pub(crate) fn read(key: &VaultKey) -> Result<SecretValue, VaultError> {
  entry::read(credential(key))
}

pub(crate) fn delete(key: &VaultKey) -> Result<(), VaultError> {
  entry::delete(credential(key))
}

fn credential(key: &VaultKey) -> keyring_core::Result<Entry> {
  Store::new()?.build(key.service(), key.account(), None)
}
