use std::{fs, path::PathBuf, sync::Arc};

use orchester_core::integration::local::{Checkout, LocalIntegrationStore, LocalPhase};
use orchester_exec::local_git::LocalGit;
use orchester_store::ProjectDb;
use orchester_workspace::tasks::local_integration::LocalIntegration;

use super::support::{Fixture, git};

fn task_git(fixture: &Fixture) -> LocalGit {
  LocalGit::new(
    &fixture.task_root,
    &fixture.task_root.join(".orchester/local-integration"),
  )
  .expect("task Git")
}

fn conflicted(fixture: &Fixture, preserve: bool) -> Checkout {
  fs::write(fixture.root().join("task.txt"), b"main conflict\n").expect("main conflict");
  git(fixture.root(), &["commit", "-am", "main conflict"]);
  let mut session = fixture.open();
  if preserve {
    fs::write(fixture.root().join("local.txt"), b"staged original\n").expect("staged");
    git(fixture.root(), &["add", "local.txt"]);
    fs::write(fixture.root().join("local.txt"), b"unstaged original\n").expect("unstaged");
    fs::write(fixture.root().join("untracked.bin"), [0, 255, 1]).expect("untracked");
    fs::write(fixture.root().join("ignored.txt"), b"ignored original\n").expect("ignored");
    session.refresh().expect("review local work");
    session.preserve("Before interrupted abort").expect("preserve");
  } else {
    session.prepare_clean().expect("prepare");
  }
  let before = task_git(fixture).snapshot().expect("original task");
  session.sync().expect("task conflict");
  assert_eq!(session.operation().phase(), LocalPhase::TaskConflicts);
  before
}

fn save_abort_intent(fixture: &Fixture) -> Checkout {
  let session = fixture.open();
  let task = task_git(fixture);
  let before = task.snapshot().expect("conflicted task");
  let result = task.preview_task_abort(&before).expect("exact abort result");
  let mut operation = session.operation().clone();
  operation.plan_task_abort(result.clone()).expect("abort intent");
  operation.advance_version();
  assert!(
    ProjectDb::open(fixture.root())
      .expect("db")
      .save_local_operation(&operation, Some(session.operation().version()))
      .expect("persist intent")
  );
  result
}

#[test]
fn local_integration_abort_completion_before_journal_update_recovers_preservation_and_releases_checkout() {
  let fixture = Fixture::new(false);
  let original_task = conflicted(&fixture, true);
  let session = fixture.open();
  let generation = session.operation().version();
  let marker = PathBuf::from(
    task_git(&fixture)
      .text(&["rev-parse", "--path-format=absolute", "--git-path", "MERGE_HEAD"])
      .expect("owned merge marker"),
  );
  drop(session);
  // Fail the production path only after Git has removed its marker, before completion can be saved.
  let authorize = Arc::new(move || {
    if marker.exists() {
      Ok(())
    } else {
      Err(orchester_core::integration::local::LocalError::AuthorityChanged)
    }
  });
  let mut session = LocalIntegration::open_authorized_at(fixture.root(), fixture.run, Some(generation), authorize)
    .expect("authorized abort");
  assert!(session.abort_task().is_err());
  drop(session);
  assert_eq!(task_git(&fixture).snapshot().expect("Git completed"), original_task);
  let saved = ProjectDb::open(fixture.root())
    .expect("db")
    .local_operation(fixture.run)
    .expect("journal")
    .expect("intent");
  assert_eq!(saved.task_abort_result(), Some(&original_task));
  assert!(saved.preservation().expect("preserved originals").retained);
  assert!(saved.reserves_checkout());
  assert!(saved.merged().is_none());
  assert_eq!(saved.phase(), LocalPhase::NeedsReconciliation);

  let mut session = fixture.open();
  session
    .reconcile()
    .expect("recognize completed abort without repeating Git");
  assert_eq!(session.operation().phase(), LocalPhase::Paused);
  assert_eq!(session.operation().owned_task(), Some(&original_task));
  assert!(session.operation().task_abort_result().is_none());
  drop(session);
  let mut session = fixture.open();
  session.reconcile().expect("resume prepared phase");
  session.cancel_preservation().expect("restore named originals");
  assert_eq!(
    fs::read(fixture.root().join("local.txt")).expect("working"),
    b"unstaged original\n"
  );
  assert_eq!(git(fixture.root(), &["show", ":local.txt"]), "staged original");
  assert_eq!(
    fs::read(fixture.root().join("untracked.bin")).expect("untracked"),
    [0, 255, 1]
  );
  assert_eq!(
    fs::read(fixture.root().join("ignored.txt")).expect("ignored"),
    b"ignored original\n"
  );
  session.remove_preservation().expect("verified recovery cleanup");
  assert!(!session.operation().reserves_checkout());
  assert!(session.operation().merged().is_none());
  assert_eq!(git(fixture.root(), &["show", "HEAD:task.txt"]), "main conflict");
}

#[test]
fn local_integration_abort_restart_before_git_resumes_only_the_persisted_owned_merge() {
  let fixture = Fixture::new(false);
  let original = conflicted(&fixture, false);
  assert_eq!(save_abort_intent(&fixture), original);
  let mut session = fixture.open();
  session.pause().expect("pause pending abort without losing intent");
  drop(session);
  let mut session = fixture.open();
  assert!(session.refresh().is_err());
  assert!(session.continue_task().is_err());
  session.reconcile().expect("finish intended abort");
  assert_eq!(task_git(&fixture).snapshot().expect("restored task"), original);
  assert!(session.operation().task_abort_result().is_none());
  drop(session);
  let mut session = fixture.open();
  session.reconcile().expect("prepared");
  session.cancel_preservation().expect("cancel preparation");
  assert!(!session.operation().reserves_checkout());
}

#[test]
fn local_integration_abort_restart_refuses_external_edits_before_and_after_git_completion() {
  for completed in [false, true] {
    let fixture = Fixture::new(false);
    conflicted(&fixture, true);
    save_abort_intent(&fixture);
    if completed {
      git(&fixture.task_root, &["merge", "--abort"]);
    }
    fs::write(fixture.task_root.join("foreign.txt"), b"external work\n").expect("external edit");
    let mut session = fixture.open();
    assert!(session.reconcile().is_err());
    assert!(session.abort_task().is_err());
    assert!(session.continue_task().is_err());
    assert!(session.refresh().is_err());
    assert!(session.operation().task_abort_result().is_some());
    assert!(session.operation().preservation().expect("retained").retained);
    assert_eq!(
      fs::read(fixture.task_root.join("foreign.txt")).expect("foreign retained"),
      b"external work\n"
    );
    assert_eq!(task_git(&fixture).operation().expect("marker").is_none(), completed);
  }
}

#[test]
fn local_integration_abort_refuses_foreign_orig_head_and_autostash_before_mutation() {
  for autostash in [false, true] {
    let fixture = Fixture::new(false);
    conflicted(&fixture, false);
    let task = task_git(&fixture);
    let before = task.snapshot().expect("owned merge");
    if autostash {
      let path = task
        .text(&["rev-parse", "--path-format=absolute", "--git-path", "MERGE_AUTOSTASH"])
        .expect("path");
      fs::write(path, format!("{}\n", before.head)).expect("foreign autostash");
    } else {
      git(&fixture.task_root, &["update-ref", "ORIG_HEAD", "HEAD^"]);
    }
    let mut session = fixture.open();
    assert!(session.abort_task().is_err());
    assert_eq!(task.snapshot().expect("unchanged task"), before);
    assert!(session.operation().task_abort_result().is_none());
  }
}
