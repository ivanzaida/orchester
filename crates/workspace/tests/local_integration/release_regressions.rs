use std::fs;

use orchester_core::integration::local::{
  LocalError, LocalIntegrationStore, LocalPath, LocalPhase, LocalRecoveryStore, RecoveryLayer,
};
use orchester_exec::local_git::{LocalGit, digest};
use orchester_store::ProjectDb;
use orchester_workspace::tasks::local_integration::{IntegrationError, LocalIntegration};

use super::{
  review_regressions::second_run,
  support::{Fixture, git},
};

fn main_git(fixture: &Fixture) -> LocalGit {
  LocalGit::new(fixture.root(), &fixture.root().join(".orchester/local-integration")).expect("main Git")
}

pub(super) fn completed_commit(fixture: &Fixture, partial: bool) {
  fs::write(fixture.root().join("local.txt"), b"selected staged\n").expect("selected");
  git(fixture.root(), &["add", "local.txt"]);
  if partial {
    fs::write(fixture.root().join("task.txt"), b"excluded staged\n").expect("staged");
    git(fixture.root(), &["add", "task.txt"]);
    fs::write(fixture.root().join("task.txt"), b"excluded working\n").expect("working");
    fs::write(fixture.root().join("untracked.txt"), b"excluded untracked\n").expect("untracked");
  }
  fs::write(fixture.root().join("ignored.txt"), b"ignored original\n").expect("ignored");
  let path = LocalPath::new("local.txt").expect("path");
  let mut first = fixture.open();
  let tree = first.selected_commit_tree(std::slice::from_ref(&path)).expect("tree");
  first.commit(&[path], &tree, "selected commit").expect("actual commit");
  assert_eq!(
    first.operation().phase(),
    if partial {
      LocalPhase::Review
    } else {
      LocalPhase::Prepared
    }
  );
  assert!(first.operation().reserves_checkout());
}

#[test]
fn local_integration_superseded_completed_commit_can_release_without_undoing_commit() {
  release_completed_commit(false);
}

#[test]
fn local_integration_superseded_partial_commit_release_retains_excluded_edits_and_recovery() {
  release_completed_commit(true);
}

fn release_completed_commit(partial: bool) {
  let fixture = Fixture::new(false);
  completed_commit(&fixture, partial);
  let actual = main_git(&fixture).snapshot().expect("completed checkout");
  let second = second_run(&fixture, true);
  assert!(matches!(
    LocalIntegration::open(fixture.root(), second),
    Err(IntegrationError::State(LocalError::ForeignOperation))
  ));
  let mut first = fixture.open();
  assert!(matches!(first.sync(), Err(IntegrationError::Run)));
  first
    .cancel_preservation()
    .expect("cancel superseded operation without preservation");
  assert_eq!(first.operation().phase(), LocalPhase::Canceled);
  assert!(!first.operation().reserves_checkout());
  assert!(first.operation().commit_tree().is_some(), "audit identity retained");
  assert_eq!(main_git(&fixture).snapshot().expect("no checkout mutation"), actual);
  assert_eq!(
    fs::read(fixture.root().join("ignored.txt")).expect("ignored"),
    b"ignored original\n"
  );
  let hash = digest(b"selected staged\n");
  let retained = LocalPath::new(&format!("retained-results/{}", hash.as_str())).expect("recovery path");
  let db = ProjectDb::open(fixture.root()).expect("db");
  assert_eq!(
    db.local_content(fixture.run, &retained, RecoveryLayer::Result)
      .expect("retained originals"),
    Some(b"selected staged\n".to_vec())
  );
  drop(db);
  drop(first);
  let mut second = LocalIntegration::open(fixture.root(), second).expect("latest usable after durable release");
  assert_eq!(second.operation().reviewed(), &actual);
  if partial {
    second
      .preserve("Latest recovery")
      .expect("preserve excluded exact versions");
  } else {
    second.prepare_clean().expect("prepare");
  }
  second.sync().expect("latest sync and fresh Check");
  assert_eq!(second.operation().phase(), LocalPhase::AwaitingApproval);
}

#[test]
fn local_integration_superseded_clean_sync_release_survives_restart_and_preserves_task_commit() {
  let fixture = Fixture::new(false);
  // Force a real synchronization commit, rather than an already-up-to-date sync.
  fs::write(fixture.root().join("main-new.txt"), b"main change\n").expect("main edit");
  git(fixture.root(), &["add", "main-new.txt"]);
  git(fixture.root(), &["commit", "-m", "main change"]);
  let mut first = fixture.open();
  first.prepare_clean().expect("clean prepare");
  first.sync().expect("sync and fresh Check");
  assert_eq!(first.operation().phase(), LocalPhase::AwaitingApproval);
  let main = main_git(&fixture).snapshot().expect("main");
  let task = git(&fixture.task_root, &["rev-parse", "HEAD"]);
  let checked = first.operation().checked().expect("exact").clone();
  drop(first);
  let second = second_run(&fixture, true);
  assert!(matches!(
    LocalIntegration::open(fixture.root(), second),
    Err(IntegrationError::State(LocalError::ForeignOperation))
  ));
  let mut first = fixture.open();
  assert!(matches!(first.approve_and_merge(&checked), Err(IntegrationError::Run)));
  first
    .cancel_preservation()
    .expect("protected caller can cancel without latest-run requirement");
  assert!(first.operation().checked().is_none(), "canceled approval invalidated");
  drop(first);
  let db = ProjectDb::open(fixture.root()).expect("restart journal");
  let saved = db.local_operation(fixture.run).expect("read").expect("saved");
  assert_eq!(saved.phase(), LocalPhase::Canceled);
  assert!(!saved.reserves_checkout());
  drop(db);
  assert_eq!(main_git(&fixture).snapshot().expect("unchanged main"), main);
  assert_eq!(git(&fixture.task_root, &["rev-parse", "HEAD"]), task);
  let mut second = LocalIntegration::open(fixture.root(), second).expect("latest usable");
  second.prepare_clean().expect("prepare");
  second.sync().expect("latest actual Check");
  assert_eq!(second.operation().phase(), LocalPhase::AwaitingApproval);
}

#[test]
fn local_integration_release_refuses_stale_main_and_foreign_task_without_mutation() {
  let fixture = Fixture::new(false);
  let mut first = fixture.open();
  first.prepare_clean().expect("prepare");
  first.sync().expect("check");
  fs::write(fixture.root().join("untracked.txt"), b"external main work\n").expect("main edit");
  let main = main_git(&fixture).snapshot().expect("external main");
  assert!(matches!(
    first.cancel_preservation(),
    Err(IntegrationError::State(LocalError::Stale))
  ));
  assert_eq!(main_git(&fixture).snapshot().expect("retained"), main);
  first.refresh().expect("explicitly review new main state");
  fs::write(fixture.task_root.join("foreign.txt"), b"external task work\n").expect("task edit");
  let task = git(&fixture.task_root, &["status", "--porcelain"]);
  assert!(matches!(
    first.cancel_preservation(),
    Err(IntegrationError::State(LocalError::ForeignOperation))
  ));
  assert!(first.operation().reserves_checkout());
  assert_eq!(git(&fixture.task_root, &["status", "--porcelain"]), task);
  assert_eq!(main_git(&fixture).snapshot().expect("retained main"), main);
}
