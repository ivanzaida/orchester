use std::{fs, path::PathBuf, sync::Arc};

use orchester_core::integration::local::{Checkout, LocalIntegrationStore, LocalPhase, Resolution};
use orchester_exec::local_git::{LocalGit, digest};
use orchester_store::ProjectDb;
use orchester_workspace::tasks::local_integration::{IntegrationError, LocalIntegration};

use super::support::{Fixture, git};

fn task_git(fixture: &Fixture) -> LocalGit {
  LocalGit::new(
    &fixture.task_root,
    &fixture.task_root.join(".orchester/local-integration"),
  )
  .expect("task Git")
}

fn preserve(fixture: &Fixture) {
  fs::write(fixture.root().join("local.txt"), b"staged original\n").expect("staged");
  git(fixture.root(), &["add", "local.txt"]);
  fs::write(fixture.root().join("local.txt"), b"unstaged original\n").expect("unstaged");
  fs::write(fixture.root().join("untracked.bin"), [0, 255, 1]).expect("untracked");
  fixture.open().preserve("Before ignore transition").expect("preserve");
}

fn cleanup(fixture: &Fixture) {
  let mut session = fixture.open();
  session.reconcile().expect("reconcile prepared checkout");
  session.cancel_preservation().expect("restore originals");
  assert_eq!(
    fs::read(fixture.root().join("local.txt")).expect("working"),
    b"unstaged original\n"
  );
  assert_eq!(git(fixture.root(), &["show", ":local.txt"]), "staged original");
  assert_eq!(
    fs::read(fixture.root().join("untracked.bin")).expect("untracked"),
    [0, 255, 1]
  );
  session.remove_preservation().expect("verified cleanup");
  assert!(!session.operation().reserves_checkout());
  assert!(session.operation().merged().is_none());
}

fn resolved_ignore_conflict(fixture: &Fixture) -> Checkout {
  fs::write(
    fixture.task_root.join(".gitignore"),
    b".orchester/\nignored.txt\ntask-only.txt\n",
  )
  .expect("task ignores");
  git(&fixture.task_root, &["commit", "-am", "task ignore rules"]);
  fs::write(
    fixture.root().join(".gitignore"),
    b".orchester/\nignored.txt\nmain-only.txt\n",
  )
  .expect("main ignores");
  git(fixture.root(), &["commit", "-am", "main ignore rules"]);
  fs::write(fixture.task_root.join("ignored.txt"), b"task ignored original\n").expect("ignored");
  preserve(fixture);
  let original = task_git(fixture).snapshot().expect("original task");
  let mut session = fixture.open();
  session.sync().expect("ignore conflict");
  assert_eq!(session.operation().phase(), LocalPhase::TaskConflicts);
  let task = task_git(fixture);
  let conflict = task.conflicts().expect("conflicts").remove(0);
  assert_eq!(conflict.path.as_str(), ".gitignore");
  let hash = task.file(&conflict.path).expect("working").as_deref().map(digest);
  session
    .save_task_resolution(&conflict, hash.as_ref(), &Resolution::Text(".orchester/\n".into()))
    .expect("resolve ignores in app");
  let actual = task.snapshot().expect("resolved task");
  assert_ne!(actual.ignored, original.ignored);
  assert!(actual.files.iter().any(|file| file.path.as_str() == "ignored.txt"));
  original
}

fn save_abort_intent(fixture: &Fixture) -> Checkout {
  let session = fixture.open();
  let task = task_git(fixture);
  let before = task.snapshot().expect("before abort");
  let result = task.preview_task_abort(&before).expect("abort preview");
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
fn local_integration_ignore_transition_sync_refuses_before_mutation_and_restart_releases_preservation() {
  let fixture = Fixture::new(false);
  fs::write(fixture.root().join(".gitignore"), b".orchester/\n").expect("expose ignored");
  git(fixture.root(), &["commit", "-am", "expose ignored file"]);
  fs::write(fixture.task_root.join("ignored.txt"), b"task ignored original\n").expect("ignored");
  preserve(&fixture);
  let original = task_git(&fixture).snapshot().expect("original task");
  let mut session = fixture.open();
  assert!(matches!(
    session.sync(),
    Err(IntegrationError::Git(orchester_exec::local_git::LocalGitError::State(
      orchester_core::integration::local::LocalError::Unsupported
    )))
  ));
  assert_eq!(session.operation().phase(), LocalPhase::Prepared);
  assert!(session.operation().sync_result().is_none());
  assert_eq!(task_git(&fixture).snapshot().expect("unchanged"), original);
  drop(session);
  cleanup(&fixture);
  assert_eq!(task_git(&fixture).snapshot().expect("still unchanged"), original);
  assert_eq!(
    fs::read(fixture.task_root.join("ignored.txt")).expect("ignored retained"),
    b"task ignored original\n"
  );
}

#[test]
fn local_integration_ignore_transition_abort_restores_classification_and_allows_cleanup() {
  let fixture = Fixture::new(false);
  let original = resolved_ignore_conflict(&fixture);
  let mut session = fixture.open();
  session.abort_task().expect("abort resolved ignore conflict");
  assert_eq!(task_git(&fixture).snapshot().expect("aborted"), original);
  assert!(session.operation().task_abort_result().is_none());
  drop(session);
  cleanup(&fixture);
}

#[test]
fn local_integration_ignore_transition_abort_restart_before_and_after_git_matches_exact_prediction() {
  for completed in [false, true] {
    let fixture = Fixture::new(false);
    let original = resolved_ignore_conflict(&fixture);
    assert_eq!(save_abort_intent(&fixture), original);
    if completed {
      git(&fixture.task_root, &["merge", "--abort"]);
    }
    let mut session = fixture.open();
    session.reconcile().expect("reconcile intended abort");
    assert_eq!(task_git(&fixture).snapshot().expect("restored task"), original);
    assert!(session.operation().task_abort_result().is_none());
    drop(session);
    cleanup(&fixture);
  }
}

#[test]
fn local_integration_ignore_transition_abort_completion_before_save_recovers_without_repeating_git() {
  let fixture = Fixture::new(false);
  let original = resolved_ignore_conflict(&fixture);
  let session = fixture.open();
  let generation = session.operation().version();
  let marker = PathBuf::from(
    task_git(&fixture)
      .text(&["rev-parse", "--path-format=absolute", "--git-path", "MERGE_HEAD"])
      .expect("marker"),
  );
  drop(session);
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
  assert_eq!(task_git(&fixture).snapshot().expect("Git completed"), original);
  let saved = ProjectDb::open(fixture.root())
    .expect("db")
    .local_operation(fixture.run)
    .expect("journal")
    .expect("intent");
  assert_eq!(saved.task_abort_result(), Some(&original));
  assert!(saved.reserves_checkout());
  fixture.open().reconcile().expect("recognize completed abort");
  cleanup(&fixture);
}

#[test]
fn local_integration_ignore_transition_abort_restart_refuses_external_work_after_git() {
  let fixture = Fixture::new(false);
  resolved_ignore_conflict(&fixture);
  save_abort_intent(&fixture);
  git(&fixture.task_root, &["merge", "--abort"]);
  fs::write(fixture.task_root.join("foreign.txt"), b"foreign retained\n").expect("foreign");
  let mut session = fixture.open();
  assert!(session.reconcile().is_err());
  assert!(session.abort_task().is_err());
  assert!(session.operation().task_abort_result().is_some());
  assert!(session.operation().preservation().expect("preserved").retained);
  assert_eq!(
    fs::read(fixture.task_root.join("foreign.txt")).expect("foreign retained"),
    b"foreign retained\n"
  );
}

#[test]
fn local_integration_ignore_transition_unchanged_classification_sync_restart_continues_and_checks() {
  let fixture = Fixture::new(false);
  fs::write(
    fixture.root().join(".gitignore"),
    b".orchester/\nignored.txt\nnew-ignored.txt\n",
  )
  .expect("main ignores");
  git(fixture.root(), &["commit", "-am", "additional ignore rule"]);
  fs::write(fixture.task_root.join("ignored.txt"), b"task ignored original\n").expect("ignored");
  preserve(&fixture);
  let session = fixture.open();
  let task = task_git(&fixture);
  let before = task.snapshot().expect("before sync");
  let result = task
    .preview_sync(&before, &session.operation().reviewed().head)
    .expect("preview");
  assert_eq!(result.ignored, before.ignored);
  let mut operation = session.operation().clone();
  operation.begin_sync(session.operation().reviewed()).expect("intent");
  operation.own_task(before);
  operation
    .plan_sync_result(result.clone())
    .expect("expected sync result");
  operation.advance_version();
  assert!(
    ProjectDb::open(fixture.root())
      .expect("db")
      .save_local_operation(&operation, Some(session.operation().version()))
      .expect("save intent")
  );
  git(
    &fixture.task_root,
    &[
      "-c",
      "merge.conflictStyle=merge",
      "merge",
      "--no-ff",
      "--no-commit",
      "--no-overwrite-ignore",
      &operation.reviewed().head,
    ],
  );
  assert_eq!(task.snapshot().expect("actual sync result"), result);
  drop(session);
  let mut session = fixture.open();
  session.reconcile().expect("recover sync");
  session.continue_task().expect("commit sync and fresh Check");
  assert_eq!(session.operation().phase(), LocalPhase::AwaitingApproval);
  assert!(session.operation().merged().is_none());
  assert_eq!(
    fs::read(fixture.task_root.join("ignored.txt")).expect("ignored retained"),
    b"task ignored original\n"
  );
}
