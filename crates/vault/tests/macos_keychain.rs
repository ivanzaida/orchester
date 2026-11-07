//! The vault against the login keychain of the user running the tests. Each test works on its own throwaway
//! item (random project and secret ids, a fake value) and removes it when it ends. The check that a value
//! exists is a search that loads attributes only, so it must not need the person to allow anything.

#![cfg(target_os = "macos")]

use orchester_core::{
  id::{ProjectId, SecretId},
  secret::{SecretValue, SecretVault, VaultKey},
};
use orchester_vault::KeychainVault;

/// A throwaway keychain item, removed when dropped.
struct Throwaway(VaultKey);

impl Drop for Throwaway {
  fn drop(&mut self) {
    // Cleanup of a test item: a failure here must not hide the assertion that failed first.
    let _ = KeychainVault.delete(&self.0);
  }
}

#[test]
fn macos_keychain_tells_a_value_exists_without_reading_it() {
  let item = Throwaway(VaultKey::new(ProjectId::generate(), SecretId::generate()));
  assert_eq!(KeychainVault.exists(&item.0), Ok(false));

  KeychainVault
    .store(
      &item.0,
      &SecretValue::new("fake-value-1".to_owned()).expect("not blank"),
    )
    .expect("the value is stored");
  assert_eq!(KeychainVault.exists(&item.0), Ok(true));

  KeychainVault.delete(&item.0).expect("the value is removed");
  assert_eq!(KeychainVault.exists(&item.0), Ok(false));
}

#[test]
fn macos_keychain_does_not_mix_up_items_that_share_a_service() {
  let project = ProjectId::generate();
  let stored = Throwaway(VaultKey::new(project, SecretId::generate()));
  let neighbour = VaultKey::new(project, SecretId::generate());

  KeychainVault
    .store(
      &stored.0,
      &SecretValue::new("fake-value-2".to_owned()).expect("not blank"),
    )
    .expect("the value is stored");

  assert_eq!(KeychainVault.exists(&stored.0), Ok(true));
  assert_eq!(KeychainVault.exists(&neighbour), Ok(false));
}
