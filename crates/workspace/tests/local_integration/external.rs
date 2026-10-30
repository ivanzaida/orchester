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

#[test]
fn local_integration_partial_ancestry_is_ambiguous_and_creates_no_extra_commit() {
  use orchester_core::integration::local::ResultEvidenceKind;
  let fixture = Fixture::new(false);
  let first = git(&fixture.task_root, &["rev-parse", "HEAD"]);
  fs::write(fixture.task_root.join("remaining.txt"), "not delivered\n").expect("edit");
  git(&fixture.task_root, &["add", "remaining.txt"]);
  git(&fixture.task_root, &["commit", "-m", "second result"]);
  let source = git(&fixture.task_root, &["rev-parse", "HEAD"]);
  git(fixture.root(), &["merge", "--ff-only", &first]);
  let before = git(fixture.root(), &["rev-parse", "HEAD"]);
  let evidence = external_integration(fixture.root(), fixture.run)
    .expect("evidence")
    .expect("partial");
  assert_eq!(evidence.kind, ResultEvidenceKind::PartialAncestry);
  assert_eq!(evidence.source.as_str(), source);
  assert!(matches!(
    LocalIntegration::open(fixture.root(), fixture.run),
    Err(IntegrationError::AlreadyIntegrated)
  ));
  assert_eq!(git(fixture.root(), &["rev-parse", "HEAD"]), before);
  assert!(!fixture.root().join("remaining.txt").exists());
  assert!(
    ProjectDb::open(fixture.root())
      .expect("db")
      .run_merge(fixture.run)
      .expect("record")
      .is_none()
  );
}

#[test]
fn local_integration_cherry_picked_scoped_result_is_ambiguous_not_full_ancestry() {
  use orchester_core::integration::local::ResultEvidenceKind;
  let fixture = Fixture::new(false);
  let source = git(&fixture.task_root, &["rev-parse", "HEAD"]);
  fs::write(fixture.root().join("unrelated.txt"), "main unrelated\n").expect("main");
  git(fixture.root(), &["add", "unrelated.txt"]);
  git(fixture.root(), &["commit", "-m", "unrelated"]);
  git(fixture.root(), &["cherry-pick", &source]);
  let before = git(fixture.root(), &["rev-parse", "HEAD"]);
  let evidence = external_integration(fixture.root(), fixture.run)
    .expect("read")
    .expect("equivalent");
  assert_eq!(evidence.kind, ResultEvidenceKind::EquivalentPatches);
  assert!(matches!(
    LocalIntegration::open(fixture.root(), fixture.run),
    Err(IntegrationError::AlreadyIntegrated)
  ));
  assert_eq!(git(fixture.root(), &["rev-parse", "HEAD"]), before);
}

#[test]
fn local_integration_incomplete_unrelated_main_does_not_claim_integration() {
  let fixture = Fixture::new(false);
  fs::write(fixture.root().join("unrelated.txt"), "main unrelated\n").expect("main");
  git(fixture.root(), &["add", "unrelated.txt"]);
  git(fixture.root(), &["commit", "-m", "unrelated"]);
  let before = git(fixture.root(), &["rev-parse", "HEAD"]);
  assert!(
    external_integration(fixture.root(), fixture.run)
      .expect("read")
      .is_none()
  );
  let session = fixture.open();
  assert!(session.operation().merged().is_none());
  assert_eq!(git(fixture.root(), &["rev-parse", "HEAD"]), before);
}

fn cleanup_intent(fixture: &Fixture) -> String {
  let mut session = fixture.open();
  session.preserve("Retained cleanup").expect("preserve");
  session.sync().expect("check");
  let exact = session.operation().checked().expect("result").clone();
  session.approve_and_merge(&exact).expect("merge");
  session.restore().expect("restore");
  let mut intent = session.operation().clone();
  intent.begin_cleanup().expect("intent");
  intent.advance_version();
  assert!(
    ProjectDb::open(fixture.root())
      .expect("db")
      .save_local_operation(&intent, Some(session.operation().version()))
      .expect("save")
  );
  exact.result.head
}

#[test]
fn local_integration_crash_before_delete_rechecks_index_without_repeating_merge() {
  let fixture = Fixture::new(false);
  fs::write(fixture.root().join("local.txt"), "selected staging\n").expect("edit");
  git(fixture.root(), &["add", "local.txt"]);
  fs::write(fixture.root().join("local.txt"), "original working\n").expect("edit");
  let merged = cleanup_intent(&fixture);
  git(fixture.root(), &["reset", "HEAD", "--", "local.txt"]);
  let mut session = fixture.open();
  assert!(session.reconcile().is_err());
  let db = ProjectDb::open(fixture.root()).expect("db");
  assert!(
    db.local_content(
      fixture.run,
      &LocalPath::new("local.txt").expect("path"),
      RecoveryLayer::Working
    )
    .expect("content")
    .is_some()
  );
  assert_eq!(git(fixture.root(), &["rev-parse", "HEAD"]), merged);
  assert_eq!(session.operation().phase(), LocalPhase::RemovingPreservation);
}

#[test]
fn local_integration_delete_before_final_journal_save_finishes_only_after_actual_verification() {
  let fixture = Fixture::new(false);
  fs::write(fixture.root().join("local.txt"), "original retained\n").expect("edit");
  let merged = cleanup_intent(&fixture);
  ProjectDb::open(fixture.root())
    .expect("db")
    .remove_local_content(fixture.run)
    .expect("simulated already deleted");
  let mut session = fixture.open();
  session.reconcile().expect("verify and finish journal");
  assert_eq!(session.operation().phase(), LocalPhase::Complete);
  assert_eq!(git(fixture.root(), &["rev-parse", "HEAD"]), merged);
  assert_eq!(
    fs::read(fixture.root().join("local.txt")).expect("working"),
    b"original retained\n"
  );
  session.reconcile().expect("repeat reconciliation");
  assert_eq!(git(fixture.root(), &["rev-parse", "HEAD"]), merged);
}

#[test]
fn local_integration_external_completion_after_owned_intent_does_not_forge_a_merge_record() {
  let fixture = Fixture::new(false);
  let mut session = fixture.open();
  session.prepare_clean().expect("prepare");
  session.sync().expect("check");
  let exact = session.operation().checked().expect("checked").clone();
  let mut pending = session.operation().clone();
  pending.approve(&exact).expect("private reviewed intent");
  pending.begin_merge(&exact).expect("intent before execution");
  pending.advance_version();
  let mut db = ProjectDb::open(fixture.root()).expect("db");
  assert!(
    db.save_local_operation(&pending, Some(session.operation().version()))
      .expect("save intent")
  );
  drop(session);
  let output = std::process::Command::new("git")
    .args(["merge", "--ff-only", &exact.result.head])
    .env("GIT_REFLOG_ACTION", "external-private-regression")
    .current_dir(fixture.root())
    .output()
    .expect("external private merge");
  assert!(output.status.success());
  let mut session = fixture.open();
  assert!(
    session.reconcile().is_err(),
    "same HEAD without our actual write receipt is ambiguous"
  );
  assert!(session.operation().merged().is_none());
  assert!(db.run_merge(fixture.run).expect("historical record").is_none());
  assert!(session.refresh().is_err());
  assert!(session.sync().is_err());
  assert_eq!(git(fixture.root(), &["rev-parse", "HEAD"]), exact.result.head);
}

#[test]
fn local_integration_old_checkpoint_cannot_prove_a_changed_branch_was_fully_integrated() {
  use orchester_core::{
    id::IdempotencyKey,
    integration::local::ResultEvidenceKind,
    revision::Revision,
    run::{BranchName, NewRun},
    time::Timestamp,
  };
  let fixture = Fixture::new(false);
  let first = git(&fixture.task_root, &["rev-parse", "HEAD"]);
  git(fixture.root(), &["branch", "checkpoint-task", &first]);
  let mut db = ProjectDb::open(fixture.root()).expect("db");
  let original = db.run(fixture.run).expect("original run").expect("exists");
  let at = Timestamp::from_unix_millis;
  let mut run = db
    .create_run(
      IdempotencyKey::generate(),
      NewRun {
        task: original.task(),
        snapshot: original.snapshot().clone(),
        branch: BranchName::new("checkpoint-task").expect("branch"),
        base: original.base().clone(),
        base_branch: original.base_branch().cloned(),
        base_source: original.base_source(),
        budget: None,
        max_duration: None,
      },
      at(10),
    )
    .expect("new private fixture run");
  let events = run.start(at(11)).expect("start");
  db.append_events(&run, &events).expect("start fact");
  let execution = run.current_attempt().expect("attempt").executions()[0].id();
  let events = run.start_step(execution, at(12)).expect("step");
  db.append_events(&run, &events).expect("step fact");
  let events = run
    .record_checkpoint(execution, Revision::new(&first).expect("first result"), at(13))
    .expect("actual unfinished-run checkpoint");
  db.append_events(&run, &events).expect("checkpoint fact");
  let events = run.complete_step(execution, at(14)).expect("succeeded");
  db.append_events(&run, &events).expect("succeeded fact");
  let status = run.status().clone();
  let snapshot = run.snapshot().clone();
  git(fixture.root(), &["merge", "--ff-only", &first]);
  std::fs::write(fixture.task_root.join("remaining.txt"), b"later undelivered work").expect("private edit");
  git(&fixture.task_root, &["add", "remaining.txt"]);
  git(&fixture.task_root, &["commit", "-m", "later result"]);
  let before = git(fixture.root(), &["rev-parse", "HEAD"]);
  let source = git(&fixture.task_root, &["rev-parse", "HEAD"]);
  git(fixture.root(), &["update-ref", "refs/heads/checkpoint-task", &source]);
  let evidence = external_integration(fixture.root(), run.id())
    .expect("preflight")
    .expect("ambiguous");
  assert_eq!(evidence.kind, ResultEvidenceKind::UnknownSource);
  assert_eq!(git(fixture.root(), &["rev-parse", "HEAD"]), before);
  assert_eq!(git(fixture.root(), &["rev-parse", "checkpoint-task"]), source);
  assert!(db.run_merge(run.id()).expect("no forged historical merge").is_none());
  assert!(db.local_operation(run.id()).expect("no new operation").is_none());
  let after = db.run(run.id()).expect("run").expect("exists");
  assert_eq!(after.status(), &status);
  assert_eq!(after.snapshot(), &snapshot);
}
