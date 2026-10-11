//! Secret metadata failures must not leave a transaction or a writer lock on a reused handle.

use std::fs;

use orchester_core::{
  id::{ProjectId, SecretId},
  name::Name,
  project::{Project, ProjectKey},
  secret::{Environment, NewSecret, Secret, SecretKind, SecretName},
  store::SecretStore,
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};
use tempfile::TempDir;

fn fixture() -> (TempDir, ProjectDb, Secret) {
  let scratch = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.tmp/secret-transaction-tests");
  fs::create_dir_all(&scratch).expect("scratch");
  let repo = TempDir::new_in(scratch).expect("repository");
  fs::create_dir(repo.path().join(".git")).expect("git");
  let project = Project::new(
    ProjectId::generate(),
    ProjectKey::new("ORC").expect("key"),
    Name::new("Project").expect("name"),
  );
  let mut db = ProjectDb::create(repo.path(), project).expect("database");
  let secret = Secret::new(
    SecretId::generate(),
    NewSecret {
      name: SecretName::new("API_KEY").expect("name"),
      kind: SecretKind::ApiKey,
      environment: Environment::Staging,
    },
    Timestamp::from_unix_millis(1),
  );
  db.create_secret(&secret).expect("metadata");
  (repo, db, secret)
}
fn rotated(secret: &Secret) -> Secret {
  Secret::from_stored(
    secret.id(),
    NewSecret {
      name: secret.name().clone(),
      kind: secret.kind(),
      environment: secret.environment(),
    },
    secret.created_at(),
    Some(Timestamp::from_unix_millis(2)),
    None,
  )
}

#[test]
fn secret_transaction_rolls_back_and_releases_writer_after_error() {
  let (repo, mut db, original) = fixture();
  let rotated = rotated(&original);
  let result = db.with_write_transaction(
    |db| {
      db.save_secret(&rotated)?;
      Err::<(), _>(StoreError::SecretNotFound(SecretId::generate()))
    },
    |error| error,
    |(), error| error,
  );
  assert!(matches!(result, Err(StoreError::SecretNotFound(_))));
  assert_eq!(db.secret(original.id()).expect("read"), Some(original.clone()));
  db.save_secret(&rotated).expect("same handle is outside transaction");
  let mut second = ProjectDb::open(repo.path()).expect("second handle");
  second.save_secret(&original).expect("writer lock released");
}

#[test]
fn secret_transaction_rolls_back_and_releases_writer_after_unwind() {
  let (repo, mut db, original) = fixture();
  let rotated = rotated(&original);
  let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
    db.with_write_transaction(
      |db| -> Result<(), StoreError> {
        db.save_secret(&rotated)?;
        panic!("fixture callback failure");
      },
      |error| error,
      |(), error| error,
    )
  }));
  assert!(result.is_err());
  assert_eq!(db.secret(original.id()).expect("read"), Some(original));
  let mut second = ProjectDb::open(repo.path()).expect("second handle");
  second.save_secret(&rotated).expect("writer lock released after unwind");
}

#[test]
fn secret_transaction_commit_failure_receives_outcome_and_releases_reused_handle() {
  let (repo, mut db, original) = fixture();
  let rotated = rotated(&original);
  let raw = rusqlite::Connection::open(repo.path().join(".orchester/orchester.db")).expect("raw handle");
  raw
    .execute_batch(
      "CREATE TABLE commit_parent (id INTEGER PRIMARY KEY);
       CREATE TABLE commit_child (
         parent_id INTEGER REFERENCES commit_parent(id) DEFERRABLE INITIALLY DEFERRED
       );
       CREATE TRIGGER fail_rotation_commit AFTER UPDATE OF rotated_at ON secrets
       BEGIN INSERT INTO commit_child VALUES (1); END;",
    )
    .expect("deferred commit failure");
  let result = db.with_write_transaction(
    |db| {
      db.save_secret(&rotated)?;
      assert_eq!(db.secret(original.id())?, Some(rotated.clone()), "UPDATE succeeded");
      Ok(false)
    },
    |_| panic!("BEGIN must succeed"),
    |external_value_changed, error| {
      assert!(!external_value_changed, "commit mapper receives the callback's outcome");
      error
    },
  );
  assert!(matches!(result, Err(StoreError::Sqlite(
    rusqlite::Error::SqliteFailure(cause, _)
  )) if cause.extended_code == rusqlite::ffi::SQLITE_CONSTRAINT_FOREIGNKEY));
  assert_eq!(db.secret(original.id()).expect("read"), Some(original.clone()));
  raw
    .execute_batch("DROP TRIGGER fail_rotation_commit")
    .expect("writer lock released");
  db.save_secret(&rotated).expect("same handle rolled back failed COMMIT");
  let mut second = ProjectDb::open(repo.path()).expect("second handle");
  second.save_secret(&original).expect("second handle can commit");
}
