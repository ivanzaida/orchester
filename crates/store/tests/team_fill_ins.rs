//! What this computer fills in for the team's configuration. Run with `cargo test -p orchester-store --test
//! team_fill_ins fill_in`.

use std::{collections::HashMap, fs};

use orchester_core::{
  id::{AgentAccountId, ConnectionId, ProjectId, ResourceId},
  name::Name,
  project::{Project, ProjectKey},
  secret::{SecretName, SecretValue, SecretVault, VaultError, VaultKey},
};
use orchester_store::{ConnectionPick, ProjectDb, StoreError};
use parking_lot::Mutex;
use rusqlite::Connection as Sqlite;
use tempfile::TempDir;

const FAKE: &str = "fake-value-7Qx2";

#[derive(Default)]
struct MemoryVault {
  values: Mutex<HashMap<VaultKey, String>>,
}

impl SecretVault for MemoryVault {
  fn store(&self, key: &VaultKey, value: &SecretValue) -> Result<(), VaultError> {
    self.values.lock().insert(key.clone(), value.expose().to_owned());
    Ok(())
  }

  fn read(&self, key: &VaultKey) -> Result<SecretValue, VaultError> {
    let value = self.values.lock().get(key).cloned().ok_or(VaultError::NotFound)?;
    SecretValue::new(value).map_err(|_| VaultError::Failed("blank".to_owned()))
  }

  fn delete(&self, key: &VaultKey) -> Result<(), VaultError> {
    self.values.lock().remove(key);
    Ok(())
  }
}

fn project_repo() -> (TempDir, ProjectDb) {
  let repo = tempfile::tempdir().expect("temporary directory");
  fs::create_dir(repo.path().join(".git")).expect("create .git");
  let project = Project::new(
    ProjectId::generate(),
    ProjectKey::new("ORC").expect("valid key"),
    Name::new("Kontur").expect("valid name"),
  );
  let db = ProjectDb::create(repo.path(), project).expect("create project");
  (repo, db)
}

fn secret_name(value: &str) -> SecretName {
  SecretName::new(value).expect("valid name")
}

fn value() -> SecretValue {
  SecretValue::new(FAKE.to_owned()).expect("not blank")
}

#[test]
fn fill_in_connection_pick_is_kept_replaced_and_cleared() {
  let (repo, mut db) = project_repo();
  let profile = ResourceId::generate();
  assert_eq!(db.connection_pick(profile).expect("read"), None);
  let first = ConnectionPick {
    connection: ConnectionId::generate(),
    account: None,
  };
  db.set_connection_pick(profile, first).expect("set");
  assert_eq!(db.connection_pick(profile).expect("read"), Some(first));
  let second = ConnectionPick {
    connection: ConnectionId::generate(),
    account: Some(AgentAccountId::generate()),
  };
  db.set_connection_pick(profile, second).expect("replace");
  drop(db);
  let db = ProjectDb::open(repo.path()).expect("reopen");
  assert_eq!(db.connection_pick(profile).expect("read"), Some(second));
  assert_eq!(db.connection_pick(ResourceId::generate()).expect("read"), None);
  let mut db = db;
  db.clear_connection_pick(profile).expect("clear");
  db.clear_connection_pick(profile).expect("clear again");
  assert_eq!(db.connection_pick(profile).expect("read"), None);
}

#[test]
fn fill_in_mcp_server_has_no_choice_until_one_is_made() {
  let (_repo, mut db) = project_repo();
  let server = ResourceId::generate();
  assert_eq!(db.team_mcp_switch(server).expect("read"), None);
  db.set_team_mcp_on_here(server, true).expect("on");
  assert_eq!(db.team_mcp_switch(server).expect("read"), Some(true));
  assert_eq!(db.team_mcp_switch(ResourceId::generate()).expect("read"), None);
  db.set_team_mcp_on_here(server, false).expect("off");
  assert_eq!(db.team_mcp_switch(server).expect("read"), Some(false));
}

#[test]
fn fill_in_secret_value_lives_in_the_keychain_only() {
  let (repo, mut db) = project_repo();
  let vault = MemoryVault::default();
  let name = secret_name("DOCS_TOKEN");
  assert!(!db.has_team_secret_value(&name).expect("read"));
  assert!(db.team_secret_value(&vault, &name).expect("read").is_none());

  db.set_team_secret_value(&vault, &name, &value()).expect("set");
  assert!(db.has_team_secret_value(&name).expect("read"));
  let read = db.team_secret_value(&vault, &name).expect("read").expect("set");
  assert_eq!(read.expose(), FAKE);
  assert_eq!(format!("{read:?}"), "SecretValue(<redacted>)");
  assert_eq!(db.team_secret_names_set().expect("names"), vec![name.clone()]);

  let dump = fs::read(repo.path().join(".orchester").join("orchester.db")).expect("database file");
  assert!(!dump.windows(FAKE.len()).any(|window| window == FAKE.as_bytes()));
  let rows: i64 = Sqlite::open(repo.path().join(".orchester").join("orchester.db"))
    .expect("open")
    .query_row("SELECT count(*) FROM team_secret_values", [], |row| row.get(0))
    .expect("count");
  assert_eq!(rows, 1);
}

#[test]
fn fill_in_secret_value_replaced_keeps_one_handle_and_clears_both_sides() {
  let (_repo, mut db) = project_repo();
  let vault = MemoryVault::default();
  let name = secret_name("DOCS_TOKEN");
  db.set_team_secret_value(&vault, &name, &value()).expect("set");
  let other = SecretValue::new("fake-value-other".to_owned()).expect("not blank");
  db.set_team_secret_value(&vault, &name, &other).expect("replace");
  assert_eq!(vault.values.lock().len(), 1);
  let read = db.team_secret_value(&vault, &name).expect("read").expect("set");
  assert_eq!(read.expose(), "fake-value-other");

  db.clear_team_secret_value(&vault, &name).expect("clear");
  db.clear_team_secret_value(&vault, &name).expect("clear again");
  assert!(vault.values.lock().is_empty());
  assert!(!db.has_team_secret_value(&name).expect("read"));
}

#[test]
fn fill_in_secret_value_gone_from_the_keychain_is_an_error_without_the_value() {
  let (_repo, mut db) = project_repo();
  let vault = MemoryVault::default();
  let name = secret_name("DOCS_TOKEN");
  db.set_team_secret_value(&vault, &name, &value()).expect("set");
  vault.values.lock().clear();
  let error = db.team_secret_value(&vault, &name).expect_err("the value is gone");
  assert!(matches!(error, StoreError::Keychain(VaultError::NotFound)));
  assert!(!error.to_string().contains(FAKE));
}
