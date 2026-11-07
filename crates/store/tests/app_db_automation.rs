//! The application database's switches for what automation may do: each is off until a person turns
//! it on, is kept across opens and is a setting of its own.

use orchester_store::AppDb;

#[test]
fn automation_changes_permissions_only_once_a_person_turned_it_on() {
  let dir = tempfile::tempdir().expect("temporary directory");
  let path = dir.path().join("orchester.db");
  let mut db = AppDb::open(&path).expect("open");
  assert!(
    !db.automation_changes_permissions().expect("read"),
    "off until turned on"
  );
  db.set_automation_changes_permissions(true).expect("store");
  drop(db);
  let mut db = AppDb::open(&path).expect("reopen");
  assert!(db.automation_changes_permissions().expect("read"));
  db.set_automation_changes_permissions(false).expect("store");
  assert!(!db.automation_changes_permissions().expect("read"));
  assert!(
    !db.automation_answers_approvals().expect("read"),
    "a setting of its own"
  );
}

#[test]
fn orchestrator_answers_agents_is_a_legacy_value_nothing_sets_and_that_starts_off() {
  let dir = tempfile::tempdir().expect("temporary directory");
  let db = AppDb::open(&dir.path().join("orchester.db")).expect("open");
  assert!(
    !db.orchestrator_answers_agents().expect("read"),
    "off until a person's earlier choice: a project takes it once as its Let the Orchestrator drive"
  );
}
