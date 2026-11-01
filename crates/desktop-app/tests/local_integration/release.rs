//! Private input harness regressions; these do not establish live human approval.
use std::fs;

use lurq::app::events::with_synthetic_input;
use orchester_core::{
  id::IdempotencyKey,
  integration::local::{LocalIntegrationStore, LocalPath, LocalPhase},
  run::{BranchName, NewRun},
  store::RunStore,
  time::Timestamp,
};
use orchester_exec::local_git::LocalGit;
use orchester_store::ProjectDb;

use super::{
  domain_support::{Fixture, git},
  integration_support::Page,
};

fn preparation(checked: bool) -> Page {
  let fixture = Fixture::new(false);
  let mut session = fixture.open();
  if checked {
    session.prepare_clean().expect("prepare");
    session.sync().expect("real Check");
  } else {
    fs::write(fixture.root().join("local.txt"), b"completed local commit\n").expect("edit");
    git(fixture.root(), &["add", "local.txt"]);
    session.refresh().expect("review edit");
    let path = LocalPath::new("local.txt").expect("path");
    let tree = session.selected_commit_tree(std::slice::from_ref(&path)).expect("tree");
    session.commit(&[path], &tree, "local commit").expect("actual commit");
  }
  drop(session);
  let mut page = Page::new(fixture);
  page.ready("local-review-cancel");
  page
}

fn actual(page: &Page) -> orchester_core::integration::local::Checkout {
  LocalGit::new(page.root(), &page.root().join(".orchester/local-integration"))
    .expect("main")
    .snapshot()
    .expect("checkout")
}

fn release(checked: bool) {
  let mut page = preparation(checked);
  let main = actual(&page);
  let task = git(&page.fixture.task_root, &["rev-parse", "HEAD"]);
  page.click("local-review-cancel");
  assert!(
    page
      .window
      .texts("local-actions")
      .expect("copy")
      .join(" ")
      .contains("Completed commits")
  );
  page.click("local-confirm");
  assert!(!page.has("local-working"), "acknowledgement is required");
  page.click("local-confirm-scope");
  page.click("local-confirm");
  page.ready("local-restart");
  let operation = ProjectDb::open(page.root())
    .expect("db")
    .local_operation(page.run())
    .expect("journal")
    .expect("exists");
  assert_eq!(operation.phase(), LocalPhase::Canceled);
  assert!(!operation.reserves_checkout());
  assert_eq!(actual(&page), main);
  assert_eq!(git(&page.fixture.task_root, &["rev-parse", "HEAD"]), task);
}

#[test]
fn local_integration_completed_commit_release_requires_acknowledgement_and_retains_commit() {
  release(false);
}

#[test]
fn local_integration_checked_release_requires_acknowledgement_and_retains_task_result() {
  release(true);
}

#[test]
fn local_integration_release_refuses_synthetic_protected_confirmation() {
  let mut page = preparation(true);
  page.window.link.input_gate().bind_window_control(true);
  let main = actual(&page);
  page.click("local-review-cancel");
  assert!(page.window.link.input_gate().protecting());
  page.click("local-confirm-scope");
  with_synthetic_input(|| page.click("local-confirm"));
  page.until("protected release refusal", |window| {
    window.texts("local-work-error").is_some()
  });
  let operation = ProjectDb::open(page.root())
    .expect("db")
    .local_operation(page.run())
    .expect("journal")
    .expect("exists");
  assert_eq!(operation.phase(), LocalPhase::AwaitingApproval);
  assert!(operation.reserves_checkout());
  assert_eq!(actual(&page), main);
}

#[test]
fn local_integration_release_generation_change_refuses_old_confirmation() {
  let mut page = preparation(false);
  page.click("local-review-cancel");
  page.click("local-confirm-scope");
  let mut session = page.fixture.open();
  session.refresh().expect("new reviewed generation");
  drop(session);
  let main = actual(&page);
  page.click("local-confirm");
  page.until("stale generation refusal", |window| {
    window.texts("local-work-error").is_some()
  });
  assert_eq!(actual(&page), main);
  assert!(
    ProjectDb::open(page.root())
      .expect("db")
      .local_operation(page.run())
      .expect("journal")
      .expect("exists")
      .reserves_checkout()
  );
}

fn supersede(page: &Page) {
  let mut db = ProjectDb::open(page.root()).expect("db");
  let first = db.run(page.run()).expect("run").expect("exists");
  db.create_run(
    IdempotencyKey::generate(),
    NewRun {
      task: first.task(),
      snapshot: first.snapshot().clone(),
      branch: BranchName::new("newer").expect("branch"),
      base: first.base().clone(),
      base_branch: first.base_branch().cloned(),
      base_source: first.base_source(),
      budget: None,
      max_duration: None,
    },
    Timestamp::now(),
  )
  .expect("latest queued run");
}

#[test]
fn local_integration_superseded_operation_remains_reachable_in_task_tab_until_release() {
  let mut page = preparation(true);
  let main = actual(&page);
  supersede(&page);
  let project = ProjectDb::open(page.root()).expect("db").project().id();
  page.window.projects().feed(project).saved();
  page.ready("local-prior-operation");
  page.ready("local-review-cancel");
  page.click("local-review-cancel");
  page.click("local-confirm-scope");
  page.click("local-confirm");
  page.ready("local-not-ready");
  assert!(
    !page.has("local-prior-operation"),
    "latest run resumes after cancellation"
  );
  let operation = ProjectDb::open(page.root())
    .expect("db")
    .local_operation(page.run())
    .expect("journal")
    .expect("saved");
  assert_eq!(operation.phase(), LocalPhase::Canceled);
  assert!(!operation.reserves_checkout());
  assert_eq!(actual(&page), main);
}
