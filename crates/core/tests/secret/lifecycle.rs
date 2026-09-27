//! Adding a secret, replacing its value and archiving it keep the project database and the
//! keychain consistent, also when one of them fails.

use orchester_core::{
  id::{ProjectId, SecretId},
  secret::{Environment, SecretError, VaultError, add, archive, existing, replace_value},
};

use crate::support::{MemoryStore, MemoryVault, VALUE, at, new_secret, value};

struct Fixture {
  project: ProjectId,
  store: MemoryStore,
  vault: MemoryVault,
}

impl Fixture {
  fn new() -> Self {
    Self {
      project: ProjectId::generate(),
      store: MemoryStore::default(),
      vault: MemoryVault::default(),
    }
  }

  fn add(&mut self, name: &str, environment: Environment) -> SecretId {
    add(
      &mut self.store,
      &self.vault,
      self.project,
      new_secret(name, environment),
      &value(VALUE),
      at(1_000),
    )
    .expect("the secret is added")
    .id()
  }

  fn stored_value(&self, id: SecretId) -> Option<String> {
    let secret = self.store.rows.iter().find(|secret| secret.id() == id).expect("a row");
    self.vault.value(&secret.vault_key(self.project))
  }
}

#[test]
fn secret_add_puts_the_value_in_the_keychain_and_the_rest_in_the_database() {
  let mut fixture = Fixture::new();
  let id = fixture.add("STAGING_DATABASE_URL", Environment::Staging);
  let secret = &fixture.store.rows[0];
  assert_eq!(secret.id(), id);
  assert_eq!(secret.name().as_str(), "STAGING_DATABASE_URL");
  assert_eq!(secret.created_at(), at(1_000));
  assert_eq!(secret.rotated_at(), None);
  assert_eq!(fixture.stored_value(id).as_deref(), Some(VALUE));
  assert!(!format!("{:?}", fixture.store.rows).contains(VALUE));
}

#[test]
fn secret_names_are_unique_per_environment() {
  let mut fixture = Fixture::new();
  fixture.add("DEPLOY_TOKEN", Environment::Staging);
  fixture.add("DEPLOY_TOKEN", Environment::Production);
  let taken = add(
    &mut fixture.store,
    &fixture.vault,
    fixture.project,
    new_secret("DEPLOY_TOKEN", Environment::Staging),
    &value("fake-other"),
    at(2_000),
  );
  assert!(matches!(
    taken,
    Err(SecretError::Taken {
      environment: Environment::Staging,
      ..
    })
  ));
  assert_eq!(fixture.vault.len(), 2, "a refused secret leaves no value behind");
  let name = fixture.store.rows[0].name().clone();
  let rows = fixture.store.rows.clone();
  assert!(existing(&rows, &name, Environment::Staging).is_some());
  assert!(existing(&rows, &name, Environment::Development).is_none());
}

#[test]
fn secret_add_removes_the_value_when_the_database_refuses_the_secret() {
  let mut fixture = Fixture::new();
  fixture.store.fail_writes = true;
  let result = add(
    &mut fixture.store,
    &fixture.vault,
    fixture.project,
    new_secret("STAGING_TOKEN", Environment::Staging),
    &value(VALUE),
    at(1_000),
  );
  assert!(matches!(result, Err(SecretError::Store(_))));
  assert_eq!(fixture.vault.len(), 0);
}

#[test]
fn secret_add_stores_nothing_when_the_keychain_refuses_the_value() {
  let mut fixture = Fixture::new();
  fixture.vault.fail_with(VaultError::TooLong { max: 2560 });
  let result = add(
    &mut fixture.store,
    &fixture.vault,
    fixture.project,
    new_secret("STAGING_TOKEN", Environment::Staging),
    &value(VALUE),
    at(1_000),
  );
  assert!(matches!(
    result,
    Err(SecretError::Vault(VaultError::TooLong { max: 2560 }))
  ));
  assert!(fixture.store.rows.is_empty());
}

#[test]
fn secret_replace_value_rotates_it() {
  let mut fixture = Fixture::new();
  let id = fixture.add("STAGING_TOKEN", Environment::Staging);
  let replaced = replace_value(
    &mut fixture.store,
    &fixture.vault,
    fixture.project,
    id,
    &value("fake-rotated"),
    at(5_000),
  )
  .expect("the value is replaced");
  assert_eq!(replaced.rotated_at(), Some(at(5_000)));
  assert_eq!(fixture.store.rows[0].rotated_at(), Some(at(5_000)));
  assert_eq!(fixture.stored_value(id).as_deref(), Some("fake-rotated"));
}

#[test]
fn secret_replace_value_reports_a_rotation_it_could_not_record() {
  let mut fixture = Fixture::new();
  let id = fixture.add("STAGING_TOKEN", Environment::Staging);
  fixture.store.fail_writes = true;
  let result = replace_value(
    &mut fixture.store,
    &fixture.vault,
    fixture.project,
    id,
    &value("fake-rotated"),
    at(5_000),
  );
  assert!(matches!(result, Err(SecretError::RotationNotRecorded { secret, .. }) if secret == id));
  assert_eq!(fixture.stored_value(id).as_deref(), Some("fake-rotated"));
}

#[test]
fn secret_archive_removes_the_value_and_frees_the_name() {
  let mut fixture = Fixture::new();
  let id = fixture.add("STAGING_TOKEN", Environment::Staging);
  let archived = archive(&mut fixture.store, &fixture.vault, fixture.project, id, at(9_000)).expect("archived");
  assert_eq!(archived.archived_at(), Some(at(9_000)));
  assert_eq!(fixture.stored_value(id), None);
  let again = archive(&mut fixture.store, &fixture.vault, fixture.project, id, at(9_500)).expect("still archived");
  assert_eq!(again.archived_at(), Some(at(9_000)));
  let replaced = replace_value(
    &mut fixture.store,
    &fixture.vault,
    fixture.project,
    id,
    &value("fake-late"),
    at(10_000),
  );
  assert!(matches!(replaced, Err(SecretError::Archived(archived)) if archived == id));
  fixture.add("STAGING_TOKEN", Environment::Staging);
}

#[test]
fn secret_archive_keeps_the_secret_when_the_keychain_cannot_remove_the_value() {
  let mut fixture = Fixture::new();
  let id = fixture.add("STAGING_TOKEN", Environment::Staging);
  fixture
    .vault
    .fail_with(VaultError::AccessDenied("the keychain is locked".to_owned()));
  let result = archive(&mut fixture.store, &fixture.vault, fixture.project, id, at(9_000));
  assert!(matches!(result, Err(SecretError::Vault(_))));
  assert!(!fixture.store.rows[0].is_archived());
}

#[test]
fn secret_unknown_ids_are_not_found() {
  let mut fixture = Fixture::new();
  let id = SecretId::generate();
  let result = archive(&mut fixture.store, &fixture.vault, fixture.project, id, at(1));
  assert!(matches!(result, Err(SecretError::NotFound(missing)) if missing == id));
}
