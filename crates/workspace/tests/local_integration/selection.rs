use std::fs;

use orchester_core::integration::local::{
  LocalIntegrationStore, LocalPath, LocalPhase, LocalRecoveryStore, RecoveryLayer,
};
use orchester_store::ProjectDb;

use super::support::{Fixture, git};

fn path(value: &str) -> LocalPath {
  LocalPath::new(value).expect("path")
}

#[test]
fn local_integration_selected_preservation_keeps_excluded_content_and_can_extend() {
  let fixture = Fixture::new(false);
  fs::write(fixture.root().join("local.txt"), "selected staged\n").expect("edit");
  git(fixture.root(), &["add", "local.txt"]);
  fs::write(fixture.root().join("local.txt"), "selected working\n").expect("edit");
  fs::write(fixture.root().join("excluded.txt"), "excluded staged\n").expect("edit");
  git(fixture.root(), &["add", "excluded.txt"]);
  fs::write(fixture.root().join("excluded.txt"), "excluded working\n").expect("edit");
  fs::write(fixture.root().join("selected.bin"), [0, 2, 255]).expect("binary");
  fs::write(fixture.root().join("excluded.bin"), [0, 9, 255]).expect("binary");
  fs::write(fixture.root().join("ignored.txt"), "ignored original").expect("ignored");
  let mut session = fixture.open();
  session
    .preserve_selected("Selected recovery", &[path("local.txt"), path("selected.bin")])
    .expect("preserve selection");
  assert_eq!(session.operation().phase(), LocalPhase::Review);
  assert_eq!(git(fixture.root(), &["show", ":excluded.txt"]), "excluded staged");
  assert_eq!(
    fs::read(fixture.root().join("excluded.txt")).expect("working"),
    b"excluded working\n"
  );
  assert_eq!(
    fs::read(fixture.root().join("excluded.bin")).expect("untracked"),
    [0, 9, 255]
  );
  assert!(!fixture.root().join("selected.bin").exists());
  assert_eq!(
    fs::read(fixture.root().join("local.txt")).expect("selected cleared"),
    b"base\n"
  );
  assert!(session.sync().is_err());
  let db = ProjectDb::open(fixture.root()).expect("db");
  assert!(
    db.local_content(fixture.run, &path("excluded.txt"), RecoveryLayer::Working)
      .expect("content")
      .is_none()
  );
  drop(session);
  let mut session = fixture.open();
  session
    .preserve_selected("Selected recovery", &[path("excluded.txt"), path("excluded.bin")])
    .expect("extend selection");
  assert_eq!(session.operation().phase(), LocalPhase::Prepared);
  session.sync().expect("check");
  let checked = session.operation().checked().expect("checked").clone();
  session.approve_and_merge(&checked).expect("merge");
  session.restore().expect("restore");
  assert_eq!(session.operation().phase(), LocalPhase::RestorationVerified);
  assert_eq!(git(fixture.root(), &["show", ":local.txt"]), "selected staged");
  assert_eq!(git(fixture.root(), &["show", ":excluded.txt"]), "excluded staged");
  assert_eq!(
    fs::read(fixture.root().join("local.txt")).expect("working"),
    b"selected working\n"
  );
  assert_eq!(
    fs::read(fixture.root().join("excluded.txt")).expect("working"),
    b"excluded working\n"
  );
  assert_eq!(
    fs::read(fixture.root().join("selected.bin")).expect("untracked"),
    [0, 2, 255]
  );
  assert_eq!(
    fs::read(fixture.root().join("ignored.txt")).expect("ignored"),
    b"ignored original"
  );
}

#[test]
fn local_integration_partial_preservation_cancel_keeps_unselected_work_and_index() {
  let fixture = Fixture::new(false);
  fs::write(fixture.root().join("local.txt"), "selected staged\n").expect("edit");
  git(fixture.root(), &["add", "local.txt"]);
  fs::write(fixture.root().join("local.txt"), "selected working\n").expect("edit");
  fs::write(fixture.root().join("excluded.txt"), "unselected\n").expect("edit");
  git(fixture.root(), &["add", "excluded.txt"]);
  let head = git(fixture.root(), &["rev-parse", "HEAD"]);
  let mut session = fixture.open();
  session
    .preserve_selected("Partial recovery", &[path("local.txt")])
    .expect("preserve");
  session
    .cancel_preservation()
    .expect("cancel with excluded work still present");
  assert_eq!(session.operation().phase(), LocalPhase::Canceled);
  assert_eq!(git(fixture.root(), &["rev-parse", "HEAD"]), head);
  assert_eq!(git(fixture.root(), &["show", ":local.txt"]), "selected staged");
  assert_eq!(git(fixture.root(), &["show", ":excluded.txt"]), "unselected");
  assert_eq!(
    fs::read(fixture.root().join("local.txt")).expect("working"),
    b"selected working\n"
  );
  session.remove_preservation().expect("verified cleanup");
}

#[test]
fn local_integration_selected_preservation_restart_retains_only_reviewed_paths() {
  let fixture = Fixture::new(false);
  fs::write(fixture.root().join("local.txt"), "selected original\n").expect("edit");
  fs::write(fixture.root().join("excluded.txt"), "excluded original\n").expect("edit");
  let session = fixture.open();
  let mut operation = session.operation().clone();
  operation
    .begin_preserve_selected(
      &operation.reviewed().clone(),
      "Interrupted selection",
      vec![path("local.txt")],
    )
    .expect("intent");
  operation.advance_version();
  let mut db = ProjectDb::open(fixture.root()).expect("db");
  assert!(
    db.save_local_operation(&operation, Some(session.operation().version()))
      .expect("save")
  );
  drop(session);
  let mut session = fixture.open();
  session.reconcile().expect("finish selected preservation");
  assert_eq!(session.operation().phase(), LocalPhase::Review);
  assert_eq!(
    fs::read(fixture.root().join("excluded.txt")).expect("excluded"),
    b"excluded original\n"
  );
  assert!(
    db.local_content(fixture.run, &path("excluded.txt"), RecoveryLayer::Working)
      .expect("content")
      .is_none()
  );
  session.cancel_preservation().expect("restore selected");
  assert_eq!(
    fs::read(fixture.root().join("local.txt")).expect("original"),
    b"selected original\n"
  );
}
