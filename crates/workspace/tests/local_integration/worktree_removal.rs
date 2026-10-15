use std::{fs, sync::Arc};

use orchester_core::{
  integration::{
    CleanupError, Leftovers,
    local::{LocalError, LocalIntegrationStore, LocalPath, LocalPhase, LocalProcessStore},
  },
  store::{RunStore, WorktreeStore},
};
use orchester_exec::programs::Programs;
use orchester_store::ProjectDb;
use orchester_workspace::tasks::{
  integration::{IntegrationProblem, leftovers_in, remove_in},
  local_integration::{IntegrationError, LocalIntegration},
};

use super::{
  release_regressions::completed_commit,
  review_regressions::second_run,
  support::{Fixture, git},
};

fn remove(fixture: &Fixture) -> Result<(), IntegrationProblem> {
  let db = ProjectDb::open(fixture.root()).expect("db");
  let run = db.run(fixture.run).expect("read run").expect("run");
  let programs = Programs::processes();
  let left = leftovers_in(fixture.root(), &programs, &[Arc::new(run.clone())])
    .expect("actual leftovers")
    .0
    .remove(0)
    .1;
  assert_eq!(left.worktrees.len(), 1);
  assert!(!left.loses_files(), "task worktree is clean");
  remove_in(fixture.root(), &programs, &run, &left)
}

fn assert_reserved(fixture: &Fixture) {
  let db = ProjectDb::open(fixture.root()).expect("db");
  let saved = db.local_operation(fixture.run).expect("read").expect("operation");
  let task_head = git(&fixture.task_root, &["rev-parse", "HEAD"]);
  assert_eq!(
    remove(fixture),
    Err(IntegrationProblem::Cleanup(CleanupError::LocalIntegration))
  );
  let reopened = ProjectDb::open(fixture.root()).expect("restart");
  assert_eq!(reopened.local_operation(fixture.run).expect("read"), Some(saved));
  assert_eq!(reopened.worktree_removal(fixture.run).expect("read"), None);
  assert!(fixture.task_root.is_dir());
  assert_eq!(git(&fixture.task_root, &["rev-parse", "HEAD"]), task_head);
}

#[test]
fn local_integration_worktree_removal_waits_for_completed_commit_release() {
  let fixture = Fixture::new(false);
  completed_commit(&fixture, false);
  let head = git(fixture.root(), &["rev-parse", "HEAD"]);
  let second = second_run(&fixture, true);
  assert_reserved(&fixture);
  assert!(matches!(
    LocalIntegration::open(fixture.root(), second),
    Err(IntegrationError::State(LocalError::ForeignOperation))
  ));
  fixture
    .open()
    .cancel_preservation()
    .expect("protected cancellation API");
  remove(&fixture).expect("actual removal after durable release");
  assert!(!fixture.task_root.exists());
  assert_eq!(git(fixture.root(), &["rev-parse", "HEAD"]), head);
  let db = ProjectDb::open(fixture.root()).expect("restart");
  assert!(db.worktree_removal(fixture.run).expect("read").is_some());
  drop(db);
  let mut latest = LocalIntegration::open(fixture.root(), second).expect("no orphaned reservation");
  latest.prepare_clean().expect("prepare");
  latest.sync().expect("actual fresh Check");
  assert_eq!(latest.operation().phase(), LocalPhase::AwaitingApproval);
}

#[test]
fn local_integration_worktree_removal_waits_for_checked_sync_release() {
  let fixture = Fixture::new(false);
  fs::write(fixture.root().join("main-new.txt"), b"new main\n").expect("main edit");
  git(fixture.root(), &["add", "main-new.txt"]);
  git(fixture.root(), &["commit", "-m", "main change"]);
  let mut session = fixture.open();
  session.prepare_clean().expect("prepare");
  session.sync().expect("real sync commit and Check");
  let checked = session.operation().checked().expect("checked").clone();
  let task_head = git(&fixture.task_root, &["rev-parse", "HEAD"]);
  drop(session);
  let second = second_run(&fixture, true);
  assert_reserved(&fixture);
  let mut session = fixture.open();
  session.reconcile().expect("restart reconciliation remains reachable");
  session.cancel_preservation().expect("release without reverting sync");
  assert!(session.operation().checked().is_none());
  drop(session);
  assert_eq!(git(&fixture.task_root, &["rev-parse", "HEAD"]), task_head);
  remove(&fixture).expect("actual removal after cancellation");
  assert!(!fixture.task_root.exists());
  // Cleanup keeps the synchronized commit on the unmerged task branch.
  assert_eq!(git(fixture.root(), &["rev-parse", "task"]), task_head);
  let mut latest = LocalIntegration::open(fixture.root(), second).expect("checkout reusable");
  assert!(latest.approve_and_merge(&checked).is_err());
  latest.prepare_clean().expect("prepare");
  latest.sync().expect("latest fresh Check");
  assert_eq!(latest.operation().phase(), LocalPhase::AwaitingApproval);
}

#[test]
fn local_integration_worktree_removal_blocks_stale_review_reservation_before_commit() {
  let fixture = Fixture::new(false);
  fs::write(fixture.root().join("local.txt"), b"staged original\n").expect("edit");
  git(fixture.root(), &["add", "local.txt"]);
  let head = git(fixture.root(), &["rev-parse", "HEAD"]);
  let path = LocalPath::new("local.txt").expect("path");
  let review = fixture.open();
  let tree = review.selected_commit_tree(std::slice::from_ref(&path)).expect("tree");
  let mut pending = review.operation().clone();
  assert!(!pending.reserves_checkout());
  drop(review);
  remove(&fixture).expect("read-only review permits removal");
  assert!(!fixture.task_root.exists());
  let current = pending.reviewed().clone();
  let version = pending.version();
  pending
    .begin_commit(&current, "must not commit")
    .expect("stale page intent");
  pending.commit_intent(vec![path], tree).expect("selected intent");
  pending.advance_version();
  let mut db = ProjectDb::open(fixture.root()).expect("stale page connection");
  assert!(
    !db
      .save_local_operation(&pending, Some(version))
      .expect("reservation refused")
  );
  assert!(
    db.begin_local_process(fixture.run).is_err(),
    "stale page cannot launch Git"
  );
  assert!(
    db.local_process(fixture.run)
      .expect("process")
      .is_none_or(|process| process.finished)
  );
  drop(db);
  assert_eq!(git(fixture.root(), &["rev-parse", "HEAD"]), head);
  assert_eq!(git(fixture.root(), &["show", ":local.txt"]), "staged original");
  assert_eq!(
    fs::read(fixture.root().join("local.txt")).expect("bytes"),
    b"staged original\n"
  );
  let db = ProjectDb::open(fixture.root()).expect("restart");
  let saved = db.local_operation(fixture.run).expect("read").expect("review");
  assert_eq!(saved.phase(), LocalPhase::Review);
  assert!(!saved.reserves_checkout());
}

#[test]
fn local_integration_worktree_removal_waits_for_unfinished_process_even_without_reservation() {
  let fixture = Fixture::new(false);
  let session = fixture.open();
  assert!(!session.operation().reserves_checkout());
  drop(session);
  let mut db = ProjectDb::open(fixture.root()).expect("db");
  let sequence = db.begin_local_process(fixture.run).expect("pending launch");
  assert_reserved(&fixture);
  db.finish_local_process(fixture.run, sequence)
    .expect("verified process completion");
  remove(&fixture).expect("finished process releases removal");
  assert!(!fixture.task_root.exists());
}

#[test]
fn local_integration_worktree_removal_after_verified_cleanup_keeps_recorded_merge() {
  let fixture = Fixture::new(false);
  let mut session = fixture.open();
  session.prepare_clean().expect("prepare");
  session.sync().expect("Check");
  let exact = session.operation().checked().expect("checked").clone();
  session.approve_and_merge(&exact).expect("protected merge API");
  assert_eq!(session.operation().phase(), LocalPhase::Complete);
  let head = git(fixture.root(), &["rev-parse", "HEAD"]);
  drop(session);
  remove(&fixture).expect("completed integration permits removal");
  assert!(!fixture.task_root.exists());
  let mut session = fixture.open();
  session
    .reconcile()
    .expect("recorded merge recovery without task worktree");
  assert_eq!(session.operation().merged(), Some(head.as_str()));
  assert!(session.approve_and_merge(&exact).is_err());
  assert_eq!(git(fixture.root(), &["rev-parse", "HEAD"]), head);
}

// The removal record is admitted before Git runs; reservation acquisition must refuse even
// while the worktree still exists, using a second connection as a stale page would.
#[test]
fn local_integration_worktree_removal_admission_blocks_reservation_before_git_removes() {
  let fixture = Fixture::new(false);
  let mut session = fixture.open();
  session.prepare_clean().expect("prepare");
  let mut db = ProjectDb::open(fixture.root()).expect("other connection");
  let run = db.run(fixture.run).expect("read").expect("run");
  let admitted = orchester_core::time::Timestamp::now();
  assert_eq!(
    db.begin_worktree_removal(fixture.run, admitted).expect("admit removal"),
    orchester_core::integration::RemovalStart::Recorded
  );
  assert!(fixture.task_root.is_dir());
  let mut pending = session.operation().clone();
  let current = pending.reviewed().clone();
  pending.begin_sync(&current).expect("valid synchronization intent");
  let version = pending.version();
  pending.advance_version();
  assert!(!db.save_local_operation(&pending, Some(version)).expect("atomic gate"));
  assert!(matches!(
    session.sync(),
    Err(IntegrationError::State(LocalError::Stale))
  ));
  drop(session);
  let saved = db.local_operation(fixture.run).expect("read").expect("saved");
  assert!(!saved.reserves_checkout());
  db.undo_worktree_removal(fixture.run, admitted)
    .expect("Git removed nothing");
  remove_in(fixture.root(), &Programs::processes(), &run, &Leftovers::default())
    .expect("normal cleanup remains usable");
}
