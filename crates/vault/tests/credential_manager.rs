//! The vault against the real Windows Credential Manager of the user running the tests. Each test
//! works on its own throwaway credential (random project and secret ids, a fake value) and
//! removes it when it ends, also when an assertion fails. Writing and removing generic
//! credentials needs no user interaction.

#![cfg(windows)]

use std::collections::HashMap;

use keyring_core::{Entry, Error, api::CredentialStoreApi};
use orchester_core::{
  id::{ProjectId, SecretId},
  secret::{SecretValue, SecretVault, VaultError, VaultKey},
};
use orchester_vault::KeychainVault;
use windows_native_keyring_store::Store;

/// Windows' limit on a generic credential's blob (`CRED_MAX_CREDENTIAL_BLOB_SIZE`).
const BLOB_LIMIT: u32 = 2560;

/// A throwaway credential, removed from the Credential Manager when dropped.
struct Throwaway {
  key: VaultKey,
}

impl Throwaway {
  fn new() -> Self {
    Self {
      key: VaultKey::new(ProjectId::generate(), SecretId::generate()),
    }
  }

  /// The credential as the Credential Manager has it, read around the vault.
  fn stored(&self) -> Result<Vec<u8>, Error> {
    self.entry().get_secret()
  }

  fn entry(&self) -> Entry {
    let target = format!("{}/{}", self.key.service(), self.key.account());
    let modifiers = HashMap::from([("target", target.as_str())]);
    Store::new()
      .expect("the Credential Manager store")
      .build(self.key.service(), self.key.account(), Some(&modifiers))
      .expect("an entry for the throwaway credential")
  }
}

impl Drop for Throwaway {
  fn drop(&mut self) {
    // Cleanup of a test credential: one the test already removed is gone, and a failure here
    // must not hide the assertion that failed first.
    let _ = self.entry().delete_credential();
  }
}

fn value(text: &str) -> SecretValue {
  SecretValue::new(text.to_owned()).expect("not blank")
}

#[test]
fn credential_manager_stores_replaces_and_removes_a_value() {
  let credential = Throwaway::new();
  let vault = KeychainVault;

  vault
    .store(&credential.key, &value("fake-value-1"))
    .expect("the value is stored");
  assert_eq!(credential.stored().expect("the credential exists"), b"fake-value-1");

  vault
    .store(&credential.key, &value("fake-value-2"))
    .expect("the value is replaced");
  assert_eq!(credential.stored().expect("the credential exists"), b"fake-value-2");

  vault.delete(&credential.key).expect("the value is removed");
  assert!(matches!(credential.stored(), Err(Error::NoEntry)));
  vault
    .delete(&credential.key)
    .expect("removing a removed value succeeds");
}

#[test]
fn credential_manager_refuses_a_long_value_without_repeating_it() {
  let credential = Throwaway::new();
  let marker = "fake-marker-7Q2";
  let long = marker.repeat(200);

  let error = KeychainVault
    .store(&credential.key, &value(&long))
    .expect_err("the value is longer than a credential holds");

  assert_eq!(error, VaultError::TooLong { max: BLOB_LIMIT });
  assert!(!format!("{error} {error:?}").contains(marker));
  assert!(matches!(credential.stored(), Err(Error::NoEntry)));
}
