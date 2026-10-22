//! UI permission snapshots reflect actual serialized storage, and fail closed while a save is in flight.
use std::{
  sync::{Arc, Barrier},
  thread,
  time::{Duration, Instant},
};

use orchester_store::AppDb;
use orchester_workspace::workspace::{OrchestratorConsent, Workspace};

#[test]
fn automation_consent_is_unknown_until_read_and_separate_from_approvals() {
  let dir = tempfile::tempdir().expect("private consent directory");
  let path = dir.path().join("app.db");
  let mut db = AppDb::open(&path).expect("private app database");
  db.set_automation_changes_permissions(true)
    .expect("persist human opt-in");
  let workspace = Workspace::new(db);
  assert!(!workspace.automation_permission_consent(), "unknown is not a grant");
  assert!(workspace.automation_changes_permissions().expect("actual consent read"));
  assert!(workspace.automation_permission_consent());
  assert!(!workspace.automation_approval_consent());
  workspace
    .set_automation_answers_approvals(true)
    .expect("separate human approval opt-in");
  assert!(workspace.automation_approval_consent());
  workspace
    .set_automation_changes_permissions(false)
    .expect("human revocation");
  assert!(!workspace.automation_permission_consent());
  assert!(
    workspace.automation_approval_consent(),
    "permission revocation does not invent an approval change"
  );
}

#[test]
fn automation_approval_consent_is_off_from_a_persons_revocation_and_back_once_granted() {
  let dir = tempfile::tempdir().expect("private consent directory");
  let workspace = Workspace::new(AppDb::open(&dir.path().join("app.db")).expect("private database"));
  workspace.set_automation_answers_approvals(true).expect("human opt-in");
  assert!(workspace.automation_approval_consent());
  // The person's click, before the save runs: off holds even if the save never happens.
  workspace.revoke_automation_approval_consent();
  assert!(!workspace.automation_approval_consent());
  assert!(
    !workspace.automation_answers_approvals().expect("stored read"),
    "an old stored grant does not undo the click"
  );
  workspace.set_automation_answers_approvals(false).expect("saved off");
  workspace.set_automation_answers_approvals(true).expect("granted again");
  assert!(workspace.automation_approval_consent());
  assert!(workspace.automation_answers_approvals().expect("stored read"));
}

#[test]
fn window_control_is_the_builds_until_a_person_chooses_and_the_database_names_its_file() {
  let dir = tempfile::tempdir().expect("private settings directory");
  let path = dir.path().join("app.db");
  let workspace = Workspace::new(AppDb::open(&path).expect("private database"));
  assert_eq!(workspace.mcp_window_control().expect("read"), None);
  workspace.set_mcp_window_control(false).expect("a person's choice");
  assert_eq!(workspace.mcp_window_control().expect("read"), Some(false));
  workspace.set_mcp_window_control(true).expect("a person's choice");
  assert_eq!(workspace.mcp_window_control().expect("read"), Some(true));
  let file = workspace.database_path().expect("a database on disk");
  assert_eq!(
    file.canonicalize().expect("the file"),
    path.canonicalize().expect("the file")
  );
}

#[test]
fn automation_consent_read_or_write_error_invalidates_prior_grant() {
  let dir = tempfile::tempdir().expect("private consent directory");
  let path = dir.path().join("app.db");
  let workspace = Workspace::new(AppDb::open(&path).expect("private database"));
  workspace
    .set_automation_changes_permissions(true)
    .expect("human opt-in");
  assert!(workspace.automation_permission_consent());
  let external = rusqlite::Connection::open(path).expect("same private database");
  external
    .execute("DROP TABLE app_settings", [])
    .expect("actual unreadable setting fixture");
  assert!(workspace.automation_changes_permissions().is_err());
  assert!(!workspace.automation_permission_consent());
  assert!(workspace.set_automation_changes_permissions(true).is_err());
  assert!(!workspace.automation_permission_consent());
}

#[test]
fn automation_consent_revocation_is_closed_while_actual_sqlite_write_waits() {
  let dir = tempfile::tempdir().expect("private consent directory");
  let path = dir.path().join("app.db");
  let workspace = Workspace::new(AppDb::open(&path).expect("private database"));
  workspace
    .set_automation_changes_permissions(true)
    .expect("human opt-in");
  let blocker = rusqlite::Connection::open(path).expect("same private database");
  blocker
    .execute_batch("BEGIN IMMEDIATE")
    .expect("real SQLite write reservation");
  let writer = workspace.clone();
  let pending = thread::spawn(move || writer.set_automation_changes_permissions(false));
  let deadline = Instant::now() + Duration::from_secs(3);
  while workspace.automation_permission_consent() {
    assert!(
      Instant::now() < deadline,
      "revocation must stop authorizing before SQLite completes"
    );
    thread::yield_now();
  }
  assert!(!pending.is_finished(), "actual reservation still blocks the save");
  blocker.execute_batch("COMMIT").expect("release reservation");
  pending.join().expect("writer joins").expect("revocation persists");
  assert!(
    !workspace
      .automation_changes_permissions()
      .expect("persisted revocation")
  );
}

#[test]
fn automation_consent_immediate_revoke_cannot_be_regranted_by_old_stored_reads() {
  let dir = tempfile::tempdir().expect("private consent directory");
  let path = dir.path().join("app.db");
  let workspace = Workspace::new(AppDb::open(&path).expect("private database"));
  workspace
    .set_automation_changes_permissions(true)
    .expect("human opt-in");
  workspace.revoke_automation_permission_consent();
  // The human event has happened, but its async save was never dispatched (or was canceled).
  let external = AppDb::open(&path).expect("independent private read");
  assert!(external.automation_changes_permissions().expect("persisted old true"));
  assert!(
    !workspace
      .automation_changes_permissions()
      .expect("real old stored read")
  );
  assert!(
    !workspace.automation_permission_consent(),
    "old read cannot clear human revocation"
  );
  workspace
    .set_automation_changes_permissions(false)
    .expect("eventual human save");
  assert!(!workspace.automation_changes_permissions().expect("persisted false"));
  workspace
    .set_automation_changes_permissions(true)
    .expect("new explicit human grant");
  assert!(
    workspace.automation_permission_consent(),
    "only a successful explicit grant lifts revocation"
  );
}

#[test]
fn automation_consent_old_pending_grant_cannot_clear_a_later_human_revocation() {
  let dir = tempfile::tempdir().expect("private consent directory");
  let path = dir.path().join("app.db");
  let workspace = Workspace::new(AppDb::open(&path).expect("private database"));
  workspace
    .set_automation_changes_permissions(true)
    .expect("initial human grant");
  let blocker = rusqlite::Connection::open(path).expect("same private database");
  blocker
    .execute_batch("BEGIN IMMEDIATE")
    .expect("actual write reservation");
  let writer = workspace.clone();
  let pending = thread::spawn(move || writer.set_automation_changes_permissions(true));
  let deadline = Instant::now() + Duration::from_secs(3);
  while workspace.automation_permission_consent() {
    assert!(Instant::now() < deadline, "old grant must begin before revocation");
    thread::yield_now();
  }
  assert!(!pending.is_finished(), "grant is actually blocked in SQLite");
  workspace.revoke_automation_permission_consent();
  blocker.execute_batch("COMMIT").expect("release reservation");
  pending
    .join()
    .expect("writer joins")
    .expect("old persisted grant finishes");
  assert!(
    !workspace
      .automation_changes_permissions()
      .expect("real stored true reread")
  );
  assert!(
    !workspace.automation_permission_consent(),
    "older grant completion cannot undo later revocation"
  );
}

const ORCHESTRATOR_CONSENTS: [OrchestratorConsent; 4] = [
  OrchestratorConsent::PublishDrafts,
  OrchestratorConsent::AnswerAgents,
  OrchestratorConsent::MergeRuns,
  OrchestratorConsent::ContinueInterrupted,
];

#[test]
fn orchestrator_consents_are_off_until_a_person_turns_each_on() {
  let dir = tempfile::tempdir().expect("private consent directory");
  let workspace = Workspace::new(AppDb::open(&dir.path().join("app.db")).expect("private database"));
  for consent in ORCHESTRATOR_CONSENTS {
    assert!(
      !workspace.orchestrator_allows(consent).expect("read"),
      "{consent:?} is off by default"
    );
  }
  workspace
    .set_orchestrator_allows(OrchestratorConsent::AnswerAgents, true)
    .expect("human opt-in");
  assert!(
    workspace
      .orchestrator_allows(OrchestratorConsent::AnswerAgents)
      .expect("read")
  );
  assert!(
    !workspace
      .orchestrator_allows(OrchestratorConsent::MergeRuns)
      .expect("read")
      && !workspace
        .orchestrator_allows(OrchestratorConsent::PublishDrafts)
        .expect("read"),
    "each switch is its own"
  );
}

#[test]
fn orchestrator_consents_are_off_from_a_persons_revocation_before_it_is_saved() {
  let dir = tempfile::tempdir().expect("private consent directory");
  let workspace = Workspace::new(AppDb::open(&dir.path().join("app.db")).expect("private database"));
  for consent in ORCHESTRATOR_CONSENTS {
    workspace.set_orchestrator_allows(consent, true).expect("human opt-in");
    assert!(workspace.orchestrator_allows(consent).expect("read"));
    workspace.revoke_orchestrator_consent(consent);
    assert!(
      !workspace.orchestrator_allows(consent).expect("read"),
      "{consent:?}: off from the person's click, though the stored setting is still on"
    );
    workspace
      .set_orchestrator_allows(consent, true)
      .expect("turned on again");
    assert!(workspace.orchestrator_allows(consent).expect("read"));
  }
}

#[test]
fn orchestrator_consent_reads_that_overlap_each_report_the_stored_grant() {
  let dir = tempfile::tempdir().expect("private consent directory");
  let path = dir.path().join("app.db");
  let workspace = Workspace::new(AppDb::open(&path).expect("private database"));
  let consent = OrchestratorConsent::AnswerAgents;
  workspace.set_orchestrator_allows(consent, true).expect("human opt-in");
  workspace
    .set_automation_changes_permissions(true)
    .expect("a grant whose next save shows when it starts");
  let blocker = rusqlite::Connection::open(path).expect("same private database");
  blocker
    .execute_batch("BEGIN IMMEDIATE")
    .expect("real SQLite write reservation");
  // An unrelated save holds the workspace's database lock while SQLite keeps it waiting, so both reads below start
  // before either of them can read.
  let holder = workspace.clone();
  let held = thread::spawn(move || holder.set_automation_changes_permissions(true));
  let deadline = Instant::now() + Duration::from_secs(3);
  while workspace.automation_permission_consent() {
    assert!(Instant::now() < deadline, "the holding save must start");
    thread::yield_now();
  }
  let started = Arc::new(Barrier::new(3));
  let reads = [(); 2].map(|()| {
    let (reader, started) = (workspace.clone(), Arc::clone(&started));
    thread::spawn(move || {
      started.wait();
      reader.orchestrator_allows(consent)
    })
  });
  started.wait();
  assert!(!held.is_finished(), "the save actually waits in SQLite");
  blocker.execute_batch("COMMIT").expect("release reservation");
  held.join().expect("holder joins").expect("the save finishes");
  for read in reads {
    assert!(
      read.join().expect("reader joins").expect("the setting reads"),
      "a read that overlaps another still reports the person's stored grant"
    );
  }
}
