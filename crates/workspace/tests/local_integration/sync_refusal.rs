use std::{fs, process::Command, sync::Arc};

use orchester_core::{
  integration::local::{
    Checkout, LocalError, LocalIntegrationStore, LocalPath, LocalPhase, LocalRecoveryStore, RecoveryLayer,
  },
  store::MergeStore,
};
use orchester_exec::local_git::{LocalGit, LocalGitError};
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

fn collision_fixture() -> Fixture {
  let fixture = Fixture::new(false);
  fs::write(fixture.root().join(".git/info/exclude"), b"*.txt\n").expect("shared ignore rules");
  fs::create_dir(fixture.root().join("incoming")).expect("incoming directory");
  fs::write(fixture.root().join("incoming/local.txt"), b"incoming tracked\n").expect("incoming");
  git(fixture.root(), &["add", "-f", "incoming/local.txt"]);
  git(fixture.root(), &["commit", "-m", "incoming tracked"]);
  fs::write(fixture.root().join("local.txt"), b"staged original\n").expect("staged");
  git(fixture.root(), &["add", "local.txt"]);
  fs::write(fixture.root().join("local.txt"), b"unstaged original\n").expect("unstaged");
  fixture.open().preserve("Before refused sync").expect("preserve");
  fixture
}

fn add_collision(fixture: &Fixture) -> bool {
  fs::create_dir_all(fixture.task_root.join("incoming")).expect("directory");
  fs::write(fixture.task_root.join("incoming/LOCAL.txt"), [0, 255, 1]).expect("ignored original");
  fixture.task_root.join("incoming/local.txt").exists()
}

fn cleanup(fixture: &Fixture, original: &Checkout) {
  let mut session = fixture.open();
  session.reconcile().expect("reconcile");
  assert!(session.operation().can_cancel());
  assert!(matches!(
    session.abort_task(),
    Err(IntegrationError::State(LocalError::WrongPhase))
  ));
  assert_eq!(
    task_git(fixture).snapshot().expect("abort leaves task intact"),
    *original
  );
  session
    .cancel_preservation()
    .expect("restore original staging and working bytes");
  assert_eq!(
    fs::read(fixture.root().join("local.txt")).expect("working"),
    b"unstaged original\n"
  );
  assert_eq!(git(fixture.root(), &["show", ":local.txt"]), "staged original");
  assert!(session.operation().preservation().expect("preserved").retained);
  session.remove_preservation().expect("verified cleanup");
  assert!(!session.operation().reserves_checkout());
  assert!(session.operation().merged().is_none());
  let db = ProjectDb::open(fixture.root()).expect("db");
  assert!(db.run_merge(fixture.run).expect("merge record").is_none());
  assert!(
    db.local_content(
      fixture.run,
      &LocalPath::new("local.txt").expect("path"),
      RecoveryLayer::Working
    )
    .expect("retained content")
    .is_none()
  );
  assert_eq!(
    fs::read(fixture.task_root.join("incoming/LOCAL.txt")).expect("ignored retained"),
    [0, 255, 1]
  );
}

fn saved_collision_intent(fixture: &Fixture) -> Checkout {
  let session = fixture.open();
  let task = task_git(fixture);
  let before = task.snapshot().expect("before");
  let mut result = task
    .preview_sync(&before, &session.operation().reviewed().head)
    .expect("preview before collision");
  add_collision(fixture);
  let before = task
    .snapshot()
    .expect("unchanged tracked checkout with ignored collision");
  // Recreate the previous release's saved prediction: it retained ignore classification but missed the alias.
  result.ignored = before.ignored.clone();
  let mut operation = session.operation().clone();
  operation
    .begin_sync(session.operation().reviewed())
    .expect("sync intent");
  operation.own_task(before.clone());
  operation.plan_sync_result(result).expect("predicted result");
  operation.advance_version();
  assert!(
    ProjectDb::open(fixture.root())
      .expect("db")
      // Git can overwrite case aliases even with --no-overwrite-ignore. A real signature refusal
      // exercises unchanged-intent recovery on every volume without destroying the retained fixture.
      .save_local_operation(&operation, Some(session.operation().version()))
      .expect("save legacy intent")
  );
  let merged = Command::new("git")
    .args([
      "-c",
      "merge.verifySignatures=true",
      "merge",
      "--no-ff",
      "--no-commit",
      "--no-overwrite-ignore",
      &operation.reviewed().head,
    ])
    .current_dir(&fixture.task_root)
    .output()
    .expect("actual Git sync");
  assert!(!merged.status.success(), "Git refuses an unsigned incoming commit");
  assert_eq!(task.snapshot().expect("refusal leaves exact checkout"), before);
  before
}

#[test]
fn local_integration_sync_refusal_case_collision_preflight_and_cleanup_follow_filesystem() {
  let fixture = collision_fixture();
  let aliases = add_collision(&fixture);
  let before = task_git(&fixture).snapshot().expect("original task");
  let mut session = fixture.open();
  let result = session.sync();
  if aliases {
    assert!(matches!(
      result,
      Err(IntegrationError::Git(LocalGitError::State(LocalError::Unsupported)))
    ));
    assert_eq!(session.operation().phase(), LocalPhase::Prepared);
    assert!(session.operation().sync_result().is_none());
    assert_eq!(task_git(&fixture).snapshot().expect("unchanged task"), before);
  } else {
    result.expect("distinct names sync and fresh Check");
    assert_eq!(session.operation().phase(), LocalPhase::AwaitingApproval);
  }
  let actual = task_git(&fixture).snapshot().expect("task outcome");
  drop(session);
  cleanup(&fixture, &actual);
}

#[test]
fn local_integration_sync_refusal_saved_case_collision_restart_abort_cancel_and_cleanup() {
  let fixture = collision_fixture();
  let before = saved_collision_intent(&fixture);
  let mut session = fixture.open();
  session.reconcile().expect("restart reconciles facts without retry");
  assert_eq!(session.operation().phase(), LocalPhase::Prepared);
  assert!(session.operation().sync_result().is_none());
  assert_eq!(task_git(&fixture).snapshot().expect("unchanged task"), before);
  drop(session);
  cleanup(&fixture, &before);
}

#[test]
fn local_integration_sync_refusal_saved_case_collision_never_releases_foreign_task_or_main_edits() {
  for main in [false, true] {
    let fixture = collision_fixture();
    saved_collision_intent(&fixture);
    let root = if main { fixture.root() } else { &fixture.task_root };
    fs::write(root.join("foreign.bin"), b"external work\n").expect("foreign edit");
    let before = task_git(&fixture).snapshot().expect("before reconciliation");
    let mut session = fixture.open();
    assert!(session.reconcile().is_err());
    assert!(session.cancel_preservation().is_err());
    assert!(session.abort_task().is_err());
    assert_eq!(task_git(&fixture).snapshot().expect("no mutation"), before);
    assert!(session.operation().reserves_checkout());
    assert!(session.operation().preservation().expect("retained").retained);
    assert_eq!(
      fs::read(root.join("foreign.bin")).expect("foreign retained"),
      b"external work\n"
    );
  }
}

#[test]
fn local_integration_sync_refusal_after_durable_intent_releases_unchanged_checkout_without_retry() {
  let fixture = collision_fixture();
  // Keep preflight valid, then deny the mutation after its durable intent through the real authorization boundary.
  let session = fixture.open();
  let generation = session.operation().version();
  drop(session);
  let root = fixture.root().to_owned();
  let run = fixture.run;
  let authorize = Arc::new(move || {
    let saved = ProjectDb::open(&root)
      .expect("db")
      .local_operation(run)
      .expect("journal");
    if saved.is_some_and(|operation| operation.sync_result().is_some()) {
      Err(LocalError::AuthorityChanged)
    } else {
      Ok(())
    }
  });
  let before = task_git(&fixture).snapshot().expect("original");
  let mut session = LocalIntegration::open_authorized_at(fixture.root(), fixture.run, Some(generation), authorize)
    .expect("authorized open");
  assert!(session.sync().is_err());
  assert_eq!(task_git(&fixture).snapshot().expect("unchanged"), before);
  drop(session);
  let mut session = fixture.open();
  session.reconcile().expect("safe restart without executing sync");
  assert_eq!(session.operation().phase(), LocalPhase::Prepared);
  assert!(session.operation().can_cancel());
  session.cancel_preservation().expect("restore");
  session.remove_preservation().expect("cleanup");
  assert!(!session.operation().reserves_checkout());
  assert_eq!(task_git(&fixture).snapshot().expect("still unchanged"), before);
}

#[test]
fn local_integration_sync_refusal_git_signature_refusal_returns_to_preparation_and_cleans_up() {
  let fixture = collision_fixture();
  git(fixture.root(), &["config", "merge.verifySignatures", "true"]);
  let before = task_git(&fixture).snapshot().expect("original task");
  let mut session = fixture.open();
  assert!(matches!(
    session.sync(),
    Err(IntegrationError::Git(LocalGitError::Refused))
  ));
  assert_eq!(session.operation().phase(), LocalPhase::Prepared);
  assert!(session.operation().sync_result().is_none());
  assert!(session.operation().can_cancel());
  assert_eq!(task_git(&fixture).snapshot().expect("Git refusal unchanged"), before);
  drop(session);
  let mut session = fixture.open();
  session.reconcile().expect("restart does not retry refused sync");
  assert_eq!(session.operation().phase(), LocalPhase::Prepared);
  session.cancel_preservation().expect("restore");
  assert_eq!(
    fs::read(fixture.root().join("local.txt")).expect("working"),
    b"unstaged original\n"
  );
  assert_eq!(git(fixture.root(), &["show", ":local.txt"]), "staged original");
  session.remove_preservation().expect("cleanup");
  assert!(!session.operation().reserves_checkout());
  assert_eq!(task_git(&fixture).snapshot().expect("still unchanged"), before);
}
