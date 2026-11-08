//! The Orchestrator's one switch, *Let the Orchestrator drive*, each project's own (ORC-593, ORC-909): unset until
//! written for the project, then on or off; the migration that made it of the six before it takes the project's
//! *answer agents* value.
//! `cargo test -p orchester-store --test orchestrator_switches orchestrator_drives`.

mod support;

use orchester_core::store::RunStore;
use orchester_store::ProjectDb;
use rusqlite::Connection;

use self::support::{copied_at_version, started_run};

#[test]
fn orchestrator_drives_is_unset_until_written_then_on_or_off() {
  let (_repo, mut db, ..) = started_run();
  assert_eq!(db.orchestrator_drives().expect("read"), None);
  db.set_orchestrator_drives(true).expect("on");
  assert_eq!(db.orchestrator_drives().expect("read"), Some(true));
  db.set_orchestrator_drives(false).expect("off");
  assert_eq!(db.orchestrator_drives().expect("read"), Some(false));
}

#[test]
fn orchestrator_drives_copies_the_application_wide_value_once_and_keeps_the_projects_own() {
  let (_repo, mut db, ..) = started_run();
  assert!(
    db.copy_orchestrator_drives(|| Ok(true)).expect("copy"),
    "a project with none takes the application-wide value"
  );
  assert!(
    db.copy_orchestrator_drives(|| panic!("read again")).expect("kept"),
    "the project's own value is not read from the application again"
  );
  db.set_orchestrator_drives(false).expect("off");
  assert!(!db.copy_orchestrator_drives(|| Ok(true)).expect("kept"));
}

#[test]
fn orchestrator_drives_migration_turns_it_on_where_answer_agents_was_on() {
  let (repo, db, run, _execution) = started_run();
  drop(db);
  let database = repo.path().join(".orchester").join("orchester.db");
  let latest = schema_version(&database);
  for (answers, expected) in [(Some(true), Some(true)), (Some(false), Some(false)), (None, None)] {
    let old = copied_at_version(&database, latest - 1);
    let path = old.path().join(".orchester").join("orchester.db");
    Connection::open(&path)
      .expect("open the old database")
      .execute("UPDATE orchestrator_switches SET answers_agents = ?1", [answers])
      .expect("what the project held");
    let upgraded = ProjectDb::open(old.path()).expect("migrate");
    assert_eq!(schema_version(&path), latest);
    assert_eq!(upgraded.run(run.id()).expect("read").expect("run"), run, "{answers:?}");
    assert_eq!(upgraded.orchestrator_drives().expect("read"), expected, "{answers:?}");
  }
}

fn schema_version(database: &std::path::Path) -> u32 {
  Connection::open(database)
    .expect("open")
    .pragma_query_value(None, "user_version", |row| row.get(0))
    .expect("user_version")
}
