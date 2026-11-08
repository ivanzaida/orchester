//! Secret values by name.

use std::fs;

use orchester_core::{
  id::ProjectId,
  name::Name,
  project::{Project, ProjectKey},
  run::Answerer,
  secret::{SecretName, SecretValue},
};
use orchester_store::ProjectDb;
use orchester_workspace::{messages::Message, team_fill_ins::FillInError};

use crate::support::{FAKE, LockedVault, MemoryVault, Team};

#[test]
fn team_fill_ins_a_secret_value_is_set_by_name_in_the_keychain() {
  let team = Team::new();
  let vault = MemoryVault::default();
  let name = SecretName::new("STAGING_TOKEN").expect("name");
  let before = team.connection_changes();
  team
    .workspace
    .set_team_secret_value(
      &team.root,
      &vault,
      &name,
      &SecretValue::new(FAKE.to_owned()).expect("value"),
      Answerer::Person,
    )
    .expect("set");
  let db = team.db();
  assert!(db.has_team_secret_value(&name).expect("read"));
  let stored = db.team_secret_value(&vault, &name).expect("read").expect("a value");
  assert_eq!(stored.expose(), FAKE);
  assert_eq!(team.connection_changes(), before + 1);

  let database = fs::read(team.root.join(".orchester/orchester.db")).expect("database file");
  assert!(
    !database.windows(FAKE.len()).any(|window| window == FAKE.as_bytes()),
    "the project database keeps the name and a handle, never the value"
  );
}

#[test]
fn team_fill_ins_a_secret_value_never_reaches_debug_or_an_error() {
  let team = Team::new();
  let name = SecretName::new("STAGING_TOKEN").expect("name");
  let value = SecretValue::new(FAKE.to_owned()).expect("value");
  assert!(!format!("{value:?}").contains(FAKE));

  let locked = team
    .workspace
    .set_team_secret_value(&team.root, &LockedVault, &name, &value, Answerer::Person)
    .expect_err("the keychain refuses");
  let local = tempfile::tempdir().expect("temporary directory");
  fs::create_dir(local.path().join(".git")).expect("repository");
  ProjectDb::create(
    local.path(),
    Project::new(
      ProjectId::generate(),
      ProjectKey::new("LOC").expect("key"),
      Name::new("Local").expect("name"),
    ),
  )
  .expect("a local project");
  let not_team = team
    .workspace
    .set_team_secret_value(local.path(), &MemoryVault::default(), &name, &value, Answerer::Person)
    .expect_err("a local project has no team secrets");
  assert!(matches!(not_team, FillInError::NotTeamProject), "{not_team:?}");

  for error in [locked, not_team] {
    let message = error.message();
    let shown = format!("{error} {error:?} {message:?}");
    assert!(!shown.contains(FAKE), "{shown}");
  }
  assert!(
    !team.db().has_team_secret_value(&name).expect("read"),
    "a refused value leaves nothing"
  );
}

#[test]
fn team_fill_ins_a_secret_value_is_a_persons_alone() {
  let team = Team::new();
  let vault = MemoryVault::default();
  let name = SecretName::new("STAGING_TOKEN").expect("name");
  let value = SecretValue::new(FAKE.to_owned()).expect("value");
  let before = team.connection_changes();

  let refused = team
    .workspace
    .set_team_secret_value(&team.root, &vault, &name, &value, Answerer::Automation)
    .expect_err("automation does not type a secret");

  assert!(matches!(refused, FillInError::PersonOnly), "{refused:?}");
  assert_eq!(refused.message(), Message::new("teamFillIns.errors.personOnly"));
  assert!(!team.db().has_team_secret_value(&name).expect("read"));
  assert!(team.db().team_secret_value(&vault, &name).expect("read").is_none());
  assert_eq!(team.connection_changes(), before, "a refused value changes nothing");
}
