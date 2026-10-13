//! Windows Credential Manager: one generic credential per secret, whose target is
//! `orchester:<project id>/<secret id>` (a shared server's token: `orchester-server:<server id>/token`; the app's MCP
//! token: `orchester-app-mcp/<installation id>`). It is kept on
//! this computer only (`Local` persistence): it does not roam with a domain profile. Values are stored as their UTF-8
//! bytes.

use std::collections::HashMap;

use keyring_core::{Entry, api::CredentialStoreApi};
use orchester_core::secret::{SecretValue, VaultError, VaultKey};
use windows_native_keyring_store::Store;

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
  let target = format!("{}/{}", key.service(), key.account());
  let modifiers = HashMap::from([("target", target.as_str()), ("persistence", "Local")]);
  Store::new()?.build(key.service(), key.account(), Some(&modifiers))
}
