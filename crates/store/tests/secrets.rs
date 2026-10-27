//! The `secrets` table keeps a secret's metadata and never its value.

use std::{fs, path::Path};

use orchester_core::{
  id::{ProjectId, SecretId},
  name::Name,
  project::{Project, ProjectKey},
  secret::{
    Environment, NewSecret, Secret, SecretKind, SecretName, SecretValue, SecretVault, VaultError, VaultKey, add,
  },
  store::SecretStore,
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};
use tempfile::TempDir;

/// A fake value; no test uses a real credential.
const VALUE: &str = "fake-secret-value-4Hq8";

fn project_db() -> (TempDir, ProjectDb) {
  let repo = tempfile::tempdir().expect("temporary directory");
  fs::create_dir(repo.path().join(".git")).expect("create .git");
  let project = Project::new(
    ProjectId::generate(),
    ProjectKey::new("ORC").expect("valid key"),
    Name::new("Kontur").expect("valid name"),
  );
  let db = ProjectDb::create(repo.path(), project).expect("create the project database");
  (repo, db)
}

fn at(millis: i64) -> Timestamp {
  Timestamp::from_unix_millis(millis)
}

fn secret(name: &str, kind: SecretKind, environment: Environment) -> Secret {
  let new = NewSecret {
    name: SecretName::new(name).expect("valid name"),
    kind,
    environment,
  };
  Secret::new(SecretId::generate(), new, at(1_000))
}

#[test]
fn secrets_round_trip_ordered_by_name_and_environment() {
  let (_repo, mut db) = project_db();
  let deploy_production = secret("DEPLOY_TOKEN", SecretKind::DeploymentToken, Environment::Production);
  let database = secret("DATABASE_URL", SecretKind::DatabaseConnection, Environment::Staging);
  let deploy_development = secret("DEPLOY_TOKEN", SecretKind::AccessToken, Environment::Development);
  for secret in [&deploy_production, &database, &deploy_development] {
    db.create_secret(secret).expect("stored");
  }
  assert_eq!(
    db.secrets().expect("listed"),
    vec![database.clone(), deploy_development, deploy_production]
  );
  assert_eq!(db.secret(database.id()).expect("read"), Some(database));
  assert_eq!(db.secret(SecretId::generate()).expect("read"), None);
}

#[test]
fn secrets_names_are_unique_among_active_secrets_of_an_environment() {
  let (_repo, mut db) = project_db();
  let first = secret("API_KEY", SecretKind::ApiKey, Environment::Staging);
  db.create_secret(&first).expect("stored");
  let duplicate = secret("API_KEY", SecretKind::Password, Environment::Staging);
  assert!(matches!(
    db.create_secret(&duplicate),
    Err(StoreError::DuplicateSecretName {
      environment: "staging",
      ..
    })
  ));

  let archived = Secret::from_stored(
    first.id(),
    NewSecret {
      name: first.name().clone(),
      kind: first.kind(),
      environment: first.environment(),
    },
    first.created_at(),
    Some(at(2_000)),
    Some(at(3_000)),
  );
  db.save_secret(&archived).expect("saved");
  assert!(db.secrets().expect("listed").is_empty());
  assert_eq!(db.secret(first.id()).expect("read"), Some(archived.clone()));
  assert_eq!(db.archived_secrets().expect("listed"), vec![archived.clone()]);
  db.create_secret(&duplicate).expect("an archived secret frees its name");
  assert_eq!(
    db.archived_secrets().expect("listed"),
    vec![archived],
    "the active one is not archived"
  );
}

#[test]
fn secrets_saving_an_unknown_secret_fails() {
  let (_repo, mut db) = project_db();
  let unknown = secret("API_KEY", SecretKind::ApiKey, Environment::Staging);
  assert!(matches!(
    db.save_secret(&unknown),
    Err(StoreError::SecretNotFound(id)) if id == unknown.id()
  ));
}

#[test]
fn secrets_table_has_no_column_for_a_value() {
  let (repo, db) = project_db();
  drop(db);
  let sqlite = rusqlite::Connection::open(repo.path().join(".orchester").join("orchester.db")).expect("open");
  let mut statement = sqlite
    .prepare("SELECT name FROM pragma_table_info('secrets') ORDER BY cid")
    .expect("table info");
  let columns: Vec<String> = statement
    .query_map([], |row| row.get(0))
    .expect("columns")
    .collect::<Result<_, _>>()
    .expect("names");
  assert_eq!(
    columns,
    [
      "id",
      "name",
      "kind",
      "environment",
      "created_at",
      "rotated_at",
      "archived_at"
    ]
  );
}

/// A keychain that accepts a value and keeps it nowhere.
struct DiscardingVault;

impl SecretVault for DiscardingVault {
  fn store(&self, _key: &VaultKey, _value: &SecretValue) -> Result<(), VaultError> {
    Ok(())
  }

  fn read(&self, _key: &VaultKey) -> Result<SecretValue, VaultError> {
    Err(VaultError::NotFound)
  }

  fn delete(&self, _key: &VaultKey) -> Result<(), VaultError> {
    Ok(())
  }
}

fn file_contains(path: &Path, needle: &[u8]) -> bool {
  let Ok(bytes) = fs::read(path) else {
    return false;
  };
  bytes.windows(needle.len()).any(|window| window == needle)
}

#[test]
fn secrets_database_files_never_hold_the_value() {
  let (repo, mut db) = project_db();
  let project = db.project().id();
  let new = NewSecret {
    name: SecretName::new("STAGING_TOKEN").expect("valid name"),
    kind: SecretKind::AccessToken,
    environment: Environment::Staging,
  };
  let value = SecretValue::new(VALUE.to_owned()).expect("not blank");
  add(&mut db, &DiscardingVault, project, new, &value, at(1_000)).expect("added");
  drop(db);
  let directory = repo.path().join(".orchester");
  let files = ["orchester.db", "orchester.db-wal"].map(|file| directory.join(file));
  assert!(
    files.iter().any(|file| file_contains(file, b"STAGING_TOKEN")),
    "the secret itself is stored"
  );
  for file in &files {
    assert!(
      !file_contains(file, VALUE.as_bytes()),
      "{} holds the value",
      file.display()
    );
  }
}
