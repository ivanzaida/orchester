//! Leaving the team keeps what this computer filled in for the team's resources no more than the resources themselves
//! (ORC-848 D10): the connection picked for a profile, which MCP servers are on here and the secret values by name go,
//! with the values in the keychain; a local secret's value stays. `cargo test -p orchester-store --test
//! team_leaving_fill_ins team_leaving`.

mod support;

use std::{collections::HashMap, fs};

use orchester_core::{
  id::{ConnectionId, ProjectId, ResourceId, ServerId},
  project::{Project, ProjectKey},
  secret::{SecretName, SecretValue, SecretVault, VaultError, VaultKey},
  sharing::MemberRole,
};
use orchester_store::{ConnectionPick, ProjectDb, SharingLink, StoreError};
use parking_lot::Mutex;
use support::{at, name};
use tempfile::TempDir;

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

/// A vault that holds values and refuses to remove them.
struct Keeping(MemoryVault);

impl SecretVault for Keeping {
  fn store(&self, key: &VaultKey, value: &SecretValue) -> Result<(), VaultError> {
    self.0.store(key, value)
  }

  fn read(&self, key: &VaultKey) -> Result<SecretValue, VaultError> {
    self.0.read(key)
  }

  fn delete(&self, _: &VaultKey) -> Result<(), VaultError> {
    Err(VaultError::Failed("the keychain is locked".to_owned()))
  }
}

fn team_project() -> (TempDir, ProjectDb) {
  let repo = tempfile::tempdir().expect("temporary directory");
  fs::create_dir(repo.path().join(".git")).expect("create .git");
  let project = Project::new(
    ProjectId::generate(),
    ProjectKey::new("KON").expect("key"),
    name("Kontur"),
  );
  let mut db = ProjectDb::create(repo.path(), project).expect("project");
  let link = SharingLink {
    server: ServerId::generate(),
    project: ProjectId::generate(),
    key: ProjectKey::new("KON").expect("key"),
    role: Some(MemberRole::Owner),
    joined_at: at(100),
    cursor: 0,
    last_synced_at: at(100),
    removed_at: None,
    address: None,
  };
  db.start_sharing(&link, &[], (&[], &HashMap::new())).expect("shared");
  (repo, db)
}

fn token() -> SecretName {
  SecretName::new("GITHUB_TOKEN").expect("name")
}

fn value() -> SecretValue {
  SecretValue::new("fake-value-7Qx2".to_owned()).expect("not blank")
}

/// A pick, a switch and a value kept for the team's resources.
fn fill_in(db: &mut ProjectDb, vault: &dyn SecretVault) -> (ResourceId, ResourceId) {
  let (profile, server) = (ResourceId::generate(), ResourceId::generate());
  db.set_connection_pick(
    profile,
    ConnectionPick {
      connection: ConnectionId::generate(),
      account: None,
    },
  )
  .expect("a pick");
  db.set_team_mcp_on_here(server, true).expect("a switch");
  db.set_team_secret_value(vault, &token(), &value()).expect("a value");
  (profile, server)
}

#[test]
fn team_leaving_removes_what_this_computer_filled_in_for_the_teams_resources() {
  let (_repo, mut db) = team_project();
  let vault = MemoryVault::default();
  let (profile, server) = fill_in(&mut db, &vault);
  assert_eq!(vault.values.lock().len(), 1);

  db.clear_team_secret_values(&vault)
    .expect("the values go from the keychain");
  db.make_local().expect("made local");

  assert_eq!(db.connection_pick(profile).expect("read"), None);
  assert_eq!(db.team_mcp_switch(server).expect("read"), None);
  assert!(
    !db.has_team_secret_value(&token()).expect("read"),
    "no row for the name"
  );
  assert!(vault.values.lock().is_empty(), "no value left in the keychain");
}

#[test]
fn team_leaving_keeps_a_local_secrets_value_in_the_keychain() {
  let (_repo, mut db) = team_project();
  let vault = MemoryVault::default();
  fill_in(&mut db, &vault);
  let local = VaultKey::new(db.project().id(), orchester_core::id::SecretId::generate());
  vault.store(&local, &value()).expect("a local secret's value");

  db.clear_team_secret_values(&vault).expect("the team's values go");
  db.make_local().expect("made local");

  assert_eq!(vault.values.lock().len(), 1, "only the local secret's value is left");
  assert!(vault.read(&local).is_ok());
}

#[test]
fn team_leaving_removes_the_rows_even_if_nothing_was_cleared_first() {
  let (_repo, mut db) = team_project();
  let (profile, server) = fill_in(&mut db, &MemoryVault::default());
  db.make_local().expect("made local");
  assert_eq!(db.connection_pick(profile).expect("read"), None);
  assert_eq!(db.team_mcp_switch(server).expect("read"), None);
  assert!(!db.has_team_secret_value(&token()).expect("read"));
}

#[test]
fn team_leaving_stops_where_the_keychain_keeps_a_value_and_the_row_stays_to_clear_again() {
  let (_repo, mut db) = team_project();
  let vault = Keeping(MemoryVault::default());
  fill_in(&mut db, &vault);

  let refused = db.clear_team_secret_values(&vault);

  assert!(matches!(refused, Err(StoreError::Keychain(_))), "{refused:?}");
  assert!(db.has_team_secret_value(&token()).expect("read"), "the row stays");
  assert_eq!(vault.0.values.lock().len(), 1);
}
