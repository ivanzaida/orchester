use std::fs;

use orchester_core::{
  integration::local::{LocalIntegrationStore, LocalPath, LocalPhase, LocalRecoveryStore, RecoveryLayer},
  store::{MergeStore, RunStore},
};
use orchester_store::ProjectDb;
use orchester_workspace::tasks::local_integration::{IntegrationError, LocalIntegration, external_integration};

use super::support::{Fixture, git};

#[test]
fn local_integration_external_ancestry_blocks_duplicate_without_inventing_records() {
  let fixture = Fixture::new(false);
  let db = ProjectDb::open(fixture.root()).expect("db");
  let run_before = db.run(fixture.run).expect("run").expect("exists");
  let task = git(&fixture.task_root, &["rev-parse", "HEAD"]);
  git(fixture.root(), &["merge", "--ff-only", "task"]);
  fs::write(fixture.root().join("later.txt"), "later unrelated\n").expect("edit");
  git(fixture.root(), &["add", "later.txt"]);
  git(fixture.root(), &["commit", "-m", "later main work"]);
  let head = git(fixture.root(), &["rev-parse", "HEAD"]);
  let evidence = external_integration(fixture.root(), fixture.run)
    .expect("evidence")
    .expect("contained");
  assert_eq!(evidence.source.as_str(), task);
  assert_eq!(evidence.target.as_str(), head);
  assert!(matches!(
    LocalIntegration::open(fixture.root(), fixture.run),
    Err(IntegrationError::AlreadyIntegrated)
  ));
  assert_eq!(git(fixture.root(), &["rev-parse", "HEAD"]), head);
  assert_eq!(git(&fixture.task_root, &["rev-parse", "HEAD"]), task);
  assert!(db.run_merge(fixture.run).expect("merge record").is_none());
  assert!(db.local_operation(fixture.run).expect("operation").is_none());
  let run_after = db.run(fixture.run).expect("run").expect("exists");
  assert_eq!(run_after.status(), run_before.status());
  assert_eq!(run_after.snapshot(), run_before.snapshot());
  git(
    fixture.root(),
    &["worktree", "remove", fixture.task_root.to_str().expect("path")],
  );
  assert!(
    external_integration(fixture.root(), fixture.run)
      .expect("removed worktree evidence")
      .is_some()
  );
}

#[test]
fn local_integration_external_merge_after_review_blocks_sync_without_changing_task() {
  let fixture = Fixture::new(false);
  let mut session = fixture.open();
  session.prepare_clean().expect("prepare");
  let version = session.operation().version();
  let task = git(&fixture.task_root, &["rev-parse", "HEAD"]);
  git(fixture.root(), &["merge", "--ff-only", "task"]);
  assert!(matches!(session.sync(), Err(IntegrationError::AlreadyIntegrated)));
  assert_eq!(session.operation().version(), version);
  assert_eq!(session.operation().phase(), LocalPhase::Prepared);
  assert_eq!(git(&fixture.task_root, &["rev-parse", "HEAD"]), task);
  assert!(
    ProjectDb::open(fixture.root())
      .expect("db")
      .run_merge(fixture.run)
      .expect("record")
      .is_none()
  );
}

#[test]
fn local_integration_restart_cleanup_rechecks_restoration_and_retains_originals_on_drift() {
  let fixture = Fixture::new(false);
  fs::write(fixture.root().join("local.txt"), "retained original\n").expect("edit");
  let mut session = fixture.open();
  session.preserve("Before cleanup").expect("preserve");
  session.sync().expect("check");
  let checked = session.operation().checked().expect("checked").clone();
  session.approve_and_merge(&checked).expect("merge");
  session.restore().expect("restore");
  let mut operation = session.operation().clone();
  operation.begin_cleanup().expect("cleanup intent");
  operation.advance_version();
  let mut db = ProjectDb::open(fixture.root()).expect("db");
  assert!(
    db.save_local_operation(&operation, Some(session.operation().version()))
      .expect("save")
  );
  drop(session);
  fs::write(fixture.root().join("local.txt"), "external work after cleanup intent\n").expect("external");
  let mut session = fixture.open();
  assert!(session.reconcile().is_err());
  assert_eq!(session.operation().phase(), LocalPhase::RemovingPreservation);
  assert_eq!(
    db.local_content(
      fixture.run,
      &LocalPath::new("local.txt").expect("path"),
      RecoveryLayer::Working
    )
    .expect("content")
    .expect("retained"),
    b"retained original\n"
  );
  assert_eq!(git(fixture.root(), &["rev-parse", "HEAD"]), checked.result.head);
  assert_eq!(
    fs::read(fixture.root().join("local.txt")).expect("external"),
    b"external work after cleanup intent\n"
  );
}
