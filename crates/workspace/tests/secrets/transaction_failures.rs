//! Real SQLite commit failures must preserve the external credential's partial-success contract.

use std::sync::atomic::{AtomicUsize, Ordering};

use orchester_core::secret::{SecretError, SecretValue, SecretVault, VaultError, VaultKey};
use orchester_store::StoreError;
use orchester_workspace::{
  messages,
  secrets::data::{ReplacementOutcome, ReplacementPermission, SecretFailure, replace_allowed_in},
};

use super::*;

#[derive(Default)]
struct Vault {
  stored: parking_lot::Mutex<Option<SecretValue>>,
  writes: AtomicUsize,
  refuse: bool,
}

impl SecretVault for Vault {
  fn store(&self, _key: &VaultKey, value: &SecretValue) -> Result<(), VaultError> {
    if self.refuse {
      return Err(VaultError::Unsupported);
    }
    *self.stored.lock() = Some(SecretValue::new(value.expose().to_owned()).expect("fixture credential"));
    self.writes.fetch_add(1, Ordering::SeqCst);
    Ok(())
  }

  fn read(&self, _key: &VaultKey) -> Result<SecretValue, VaultError> {
    self
      .stored
      .lock()
      .as_ref()
      .map(|value| SecretValue::new(value.expose().to_owned()).expect("fixture credential"))
      .ok_or(VaultError::NotFound)
  }

  fn delete(&self, _key: &VaultKey) -> Result<(), VaultError> {
    panic!("replacement must not delete a credential");
  }
}

fn raw_db(repo: &TempDir) -> rusqlite::Connection {
  rusqlite::Connection::open(repo.path().join(".orchester/orchester.db")).expect("fixture database")
}

fn fail_rotation_commit(raw: &rusqlite::Connection) {
  // The UPDATE succeeds: this foreign key is checked only by the outer COMMIT.
  raw
    .execute_batch(
      "CREATE TABLE commit_parent (id INTEGER PRIMARY KEY);
       CREATE TABLE commit_child (
         parent_id INTEGER REFERENCES commit_parent(id) DEFERRABLE INITIALLY DEFERRED
       );
       CREATE TRIGGER fail_rotation_commit AFTER UPDATE OF rotated_at ON secrets
       BEGIN INSERT INTO commit_child VALUES (1); END;",
    )
    .expect("deferred commit failure fixture");
}

fn attempt(repo: &TempDir, vault: &Vault, id: SecretId) -> Result<ReplacementOutcome, SecretFailure> {
  replace_allowed_in(
    repo.path(),
    vault,
    id,
    &SecretValue::new("fixture-only-replacement".into()).expect("fixture credential"),
    ReplacementPermission::UnusedOnly,
  )
}

#[test]
fn secrets_replacement_commit_failure_reports_stored_value_and_releases_writer() {
  let (repo, mut db) = project_db();
  let active = secret("API_KEY");
  db.create_secret(&active).expect("metadata");
  let raw = raw_db(&repo);
  fail_rotation_commit(&raw);
  let vault = Vault::default();

  let error = attempt(&repo, &vault, active.id()).expect_err("COMMIT must fail");
  assert!(matches!(
    &error,
    SecretError::RotationNotRecorded { secret, source: StoreError::Sqlite(
      rusqlite::Error::SqliteFailure(cause, _)
    ) } if *secret == active.id() && cause.extended_code == rusqlite::ffi::SQLITE_CONSTRAINT_FOREIGNKEY
  ));
  assert_eq!(vault.writes.load(Ordering::SeqCst), 1);
  let holds_replacement = vault
    .stored
    .lock()
    .as_ref()
    .is_some_and(|value| value.expose() == "fixture-only-replacement");
  assert!(holds_replacement, "vault holds the replacement credential");
  assert_eq!(db.secret(active.id()).expect("metadata"), Some(active.clone()));
  let message = messages::secret_error(&error);
  assert_eq!(message.key, "errors.secretRotationNotRecorded");
  assert!(message.args.is_empty());
  let locale: serde_json::Value = serde_json::from_str(include_str!("../../assets/locales/en.json")).expect("locale");
  assert_eq!(
    locale["errors"]["secretRotationNotRecorded"],
    "The new value is stored, but the time of the change could not be recorded. The log has the details."
  );

  // A second handle can acquire the writer and commit after the failed replacement.
  raw
    .execute_batch("DROP TRIGGER fail_rotation_commit")
    .expect("writer lock released");
  let mut second = ProjectDb::open(repo.path()).expect("second handle");
  second.save_secret(&active).expect("subsequent write commits");
}

#[test]
fn secrets_replacement_begin_failure_does_not_claim_or_store_a_new_value() {
  let (repo, mut db) = project_db();
  let active = secret("API_KEY");
  db.create_secret(&active).expect("metadata");
  let raw = raw_db(&repo);
  raw.execute_batch("BEGIN IMMEDIATE").expect("hold competing writer");
  let vault = Vault::default();
  let result = attempt(&repo, &vault, active.id());
  raw.execute_batch("ROLLBACK").expect("release fixture writer");

  let error = result.expect_err("BEGIN must fail");
  assert!(matches!(&error, SecretError::Store(StoreError::Sqlite(
    rusqlite::Error::SqliteFailure(cause, _)
  )) if cause.code == rusqlite::ErrorCode::DatabaseBusy));
  // A writer that holds the database says so (ORC-6), rather than a general storage failure.
  assert_eq!(messages::secret_error(&error).key, "errors.databaseLocked");
  assert_eq!(vault.writes.load(Ordering::SeqCst), 0);
  assert!(vault.stored.lock().is_none());
  assert_eq!(db.secret(active.id()).expect("metadata"), Some(active));
}

#[test]
fn secrets_replacement_vault_failure_keeps_typed_error_and_releases_writer() {
  let (repo, mut db) = project_db();
  let active = secret("API_KEY");
  db.create_secret(&active).expect("metadata");
  let vault = Vault {
    refuse: true,
    ..Vault::default()
  };
  let error = attempt(&repo, &vault, active.id()).expect_err("vault must fail");
  assert!(matches!(error, SecretError::Vault(VaultError::Unsupported)));
  assert_eq!(messages::secret_error(&error).key, "errors.keychainUnsupported");
  assert_eq!(vault.writes.load(Ordering::SeqCst), 0);
  assert!(vault.stored.lock().is_none());
  assert_eq!(db.secret(active.id()).expect("metadata"), Some(active.clone()));
  let mut second = ProjectDb::open(repo.path()).expect("second handle");
  second.save_secret(&active).expect("writer lock released");
}
