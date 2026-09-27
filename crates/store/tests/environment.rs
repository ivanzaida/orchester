//! Project settings › Environment in the project database: the variables a project adds to the
//! environment of its agents and command tools.

mod support;

use std::collections::BTreeMap;

use orchester_core::{
  resource::{CommandEnvironment, VariableName},
  store::SettingsStore,
};
use orchester_store::ProjectDb;

use self::support::started_run;

#[test]
fn environment_is_empty_until_saved_then_round_trips() {
  let (repo, mut db, ..) = started_run();
  assert!(db.project_environment().expect("read").is_empty());
  let variable = |name: &str| VariableName::new(name).expect("valid name");
  let environment = CommandEnvironment::new(BTreeMap::from([
    (variable("CARGO_TARGET_DIR"), "/repo/.orchester/target".to_owned()),
    (variable("RUST_LOG"), "info".to_owned()),
  ]))
  .expect("valid variables");
  db.save_project_environment(&environment).expect("save");
  let reopened = ProjectDb::open(repo.path()).expect("open");
  assert_eq!(reopened.project_environment().expect("read"), environment);
  db.save_project_environment(&CommandEnvironment::default())
    .expect("clear");
  assert!(db.project_environment().expect("read").is_empty());
}
