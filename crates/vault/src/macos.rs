//! The macOS login keychain: one generic password per secret, with the service
//! `orchester:<project id>` and the account `<secret id>` (a shared server's token: the service
//! `orchester-server:<server id>` and the account `token`; the app's MCP token: the service `orchester-app-mcp`
//! and the account `<installation id>`). Keychain Access lists it under the
//! service. Values are stored as their UTF-8 bytes.

use std::collections::HashMap;

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

/// A search by service and account that loads the item's attributes and never its data. Reading the data is what
/// the keychain asks the person to allow (an ad-hoc signed build has a new identity each time it is built), so
/// this lookup is silent. The crate drops a hit without a service or an account attribute; the items this app writes
/// always have both.
pub(crate) fn exists(key: &VaultKey) -> Result<bool, VaultError> {
  let spec = HashMap::from([("service", key.service()), ("user", key.account())]);
  let found = Store::new()
    .and_then(|store| store.search(&spec))
    .map_err(entry::vault_error)?;
  Ok(!found.is_empty())
}

pub(crate) fn delete(key: &VaultKey) -> Result<(), VaultError> {
  entry::delete(credential(key))
}

fn credential(key: &VaultKey) -> keyring_core::Result<Entry> {
  Store::new()?.build(key.service(), key.account(), None)
}
