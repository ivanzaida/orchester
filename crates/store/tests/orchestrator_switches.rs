//! The Orchestrator's six switches, each project's own (ORC-593): unset until written for the project, then on or off,
//! each apart from the others; the migration that adds them keeps what the project held.
//! `cargo test -p orchester-store --test orchestrator_switches orchestrator_switches`.

mod support;

use orchester_core::store::RunStore;
use orchester_store::{OrchestratorSwitch, ProjectDb};
use rusqlite::Connection;

use self::support::{copied_at_version, started_run};

#[test]
fn orchestrator_switches_are_unset_until_written_then_on_or_off() {
  let (_repo, mut db, ..) = started_run();
  for switch in OrchestratorSwitch::ALL {
    assert_eq!(db.orchestrator_switch(switch).expect("read"), None, "{switch:?}");
    db.set_orchestrator_switch(switch, true).expect("on");
    assert_eq!(db.orchestrator_switch(switch).expect("read"), Some(true), "{switch:?}");
    db.set_orchestrator_switch(switch, false).expect("off");
    assert_eq!(db.orchestrator_switch(switch).expect("read"), Some(false), "{switch:?}");
  }
}

#[test]
fn orchestrator_switches_are_written_each_apart_from_the_others() {
  let (repo, mut db, ..) = started_run();
  db.set_orchestrator_switch(OrchestratorSwitch::MergeRuns, true)
    .expect("on");
  db.set_orchestrator_switch(OrchestratorSwitch::WriteGit, false)
    .expect("off");
  drop(db);
  let db = ProjectDb::open(repo.path()).expect("reopen");
  for switch in OrchestratorSwitch::ALL {
    let expected = match switch {
      OrchestratorSwitch::MergeRuns => Some(true),
      OrchestratorSwitch::WriteGit => Some(false),
      _ => None,
    };
    assert_eq!(db.orchestrator_switch(switch).expect("read"), expected, "{switch:?}");
  }
}

#[test]
fn orchestrator_switches_migration_keeps_the_project_and_leaves_every_switch_unset() {
  let (repo, db, run, _execution) = started_run();
  drop(db);
  let database = repo.path().join(".orchester").join("orchester.db");
  let latest = schema_version(&database);
  let old = copied_at_version(&database, latest - 1);
  let mut upgraded = ProjectDb::open(old.path()).expect("migrate");
  assert_eq!(
    schema_version(&old.path().join(".orchester").join("orchester.db")),
    latest
  );
  assert_eq!(upgraded.run(run.id()).expect("read").expect("run"), run);
  for switch in OrchestratorSwitch::ALL {
    assert_eq!(upgraded.orchestrator_switch(switch).expect("read"), None, "{switch:?}");
  }
  upgraded
    .set_orchestrator_switch(OrchestratorSwitch::AnswerAgents, true)
    .expect("on after the migration");
  assert_eq!(
    upgraded
      .orchestrator_switch(OrchestratorSwitch::AnswerAgents)
      .expect("read"),
    Some(true)
  );
}

fn schema_version(database: &std::path::Path) -> u32 {
  Connection::open(database)
    .expect("open")
    .pragma_query_value(None, "user_version", |row| row.get(0))
    .expect("user_version")
}
