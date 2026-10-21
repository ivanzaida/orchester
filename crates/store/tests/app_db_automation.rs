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
fn orchestrator_publishes_drafts_only_once_a_person_turned_it_on() {
  let dir = tempfile::tempdir().expect("temporary directory");
  let path = dir.path().join("orchester.db");
  let mut db = AppDb::open(&path).expect("open");
  assert!(
    !db.orchestrator_publishes_drafts().expect("read"),
    "off until turned on"
  );
  db.set_orchestrator_publishes_drafts(true).expect("store");
  drop(db);
  let mut db = AppDb::open(&path).expect("reopen");
  assert!(db.orchestrator_publishes_drafts().expect("read"));
  assert!(
    !db.automation_changes_permissions().expect("read"),
    "publishing drafts lets nothing widen"
  );
  db.set_orchestrator_publishes_drafts(false).expect("store");
  assert!(!db.orchestrator_publishes_drafts().expect("read"));
}

#[test]
fn orchestrator_answers_agents_and_merges_runs_only_once_a_person_turned_each_on() {
  let dir = tempfile::tempdir().expect("temporary directory");
  let path = dir.path().join("orchester.db");
  let mut db = AppDb::open(&path).expect("open");
  assert!(!db.orchestrator_answers_agents().expect("read"), "off until turned on");
  assert!(!db.orchestrator_merges_runs().expect("read"), "off until turned on");
  db.set_orchestrator_answers_agents(true).expect("store");
  drop(db);
  let mut db = AppDb::open(&path).expect("reopen");
  assert!(db.orchestrator_answers_agents().expect("read"));
  assert!(!db.orchestrator_merges_runs().expect("read"), "a setting of its own");
  assert!(
    !db.orchestrator_publishes_drafts().expect("read"),
    "a setting of its own"
  );
  db.set_orchestrator_merges_runs(true).expect("store");
  db.set_orchestrator_answers_agents(false).expect("store");
  assert!(db.orchestrator_merges_runs().expect("read"));
  assert!(!db.orchestrator_answers_agents().expect("read"));
}

#[test]
fn orchestrator_continues_interrupted_runs_only_once_a_person_turned_it_on() {
  let dir = tempfile::tempdir().expect("temporary directory");
  let path = dir.path().join("orchester.db");
  let mut db = AppDb::open(&path).expect("open");
  assert!(
    !db.orchestrator_continues_interrupted().expect("read"),
    "off until turned on"
  );
  db.set_orchestrator_continues_interrupted(true).expect("store");
  drop(db);
  let mut db = AppDb::open(&path).expect("reopen");
  assert!(db.orchestrator_continues_interrupted().expect("read"));
  assert!(
    !db.orchestrator_answers_agents().expect("read") && !db.orchestrator_merges_runs().expect("read"),
    "a setting of its own"
  );
  db.set_orchestrator_continues_interrupted(false).expect("store");
  assert!(!db.orchestrator_continues_interrupted().expect("read"));
}

#[test]
fn orchestrator_merges_finished_runs_only_once_a_person_turned_it_on() {
  let dir = tempfile::tempdir().expect("temporary directory");
  let path = dir.path().join("orchester.db");
  let mut db = AppDb::open(&path).expect("open");
  assert!(!db.orchestrator_merges_finished().expect("read"), "off until turned on");
  db.set_orchestrator_merges_finished(true).expect("store");
  drop(db);
  let mut db = AppDb::open(&path).expect("reopen");
  assert!(db.orchestrator_merges_finished().expect("read"));
  assert!(
    !db.orchestrator_merges_runs().expect("read") && !db.orchestrator_answers_agents().expect("read"),
    "a setting of its own: merge runs and answer agents stay as they are"
  );
  db.set_orchestrator_merges_finished(false).expect("store");
  assert!(!db.orchestrator_merges_finished().expect("read"));
}
