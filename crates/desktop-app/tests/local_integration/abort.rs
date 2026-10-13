use std::fs;

use lurq::app::events::with_synthetic_input;
use orchester_core::{
  integration::local::{LocalIntegrationStore, LocalPhase},
  store::MergeStore,
};
use orchester_exec::local_git::LocalGit;
use orchester_store::ProjectDb;

use super::{
  domain_support::{Fixture, git},
  integration_support::Page,
};

fn conflict() -> Page {
  let fixture = Fixture::new(false);
  fs::write(fixture.root().join("task.txt"), b"new main conflict\n").expect("main edit");
  git(fixture.root(), &["commit", "-am", "main conflict"]);
  fs::write(fixture.root().join("local.txt"), b"retained local changes\n").expect("local");
  let mut session = fixture.open();
  session.preserve("Private abort recovery").expect("preserve");
  session.sync().expect("owned merge conflicts");
  assert_eq!(session.operation().phase(), LocalPhase::TaskConflicts);
  drop(session);
  let mut page = Page::new(fixture);
  page.ready("local-review-abort");
  page
}

fn task_git(page: &Page) -> LocalGit {
  LocalGit::new(
    &page.fixture.task_root,
    &page.fixture.task_root.join(".orchester/local-integration"),
  )
  .expect("private task Git")
}

#[test]
fn local_integration_unresolved_task_conflicts_allow_acknowledged_scoped_abort() {
  let mut page = conflict();
  let main = git(page.root(), &["rev-parse", "HEAD"]);
  let task = git(&page.fixture.task_root, &["rev-parse", "HEAD"]);
  assert!(!task_git(&page).conflicts().expect("unresolved").is_empty());
  let unresolved = task_git(&page).snapshot().expect("conflict");
  page.click("local-confirm-scope");
  page.click("local-confirm");
  assert_eq!(
    task_git(&page).snapshot().expect("continuation remains gated"),
    unresolved
  );
  page.click("local-review-abort");
  page.click("local-confirm-scope");
  // Private headless native-input regression, never a live person's approval or an MCP bypass.
  page.click("local-confirm");
  page.until("owned abort completes", |window| {
    window
      .texts("local-recovery")
      .is_some_and(|words| words.join(" ").contains("Paused"))
  });
  assert!(task_git(&page).operation().expect("marker").is_none());
  assert_eq!(git(&page.fixture.task_root, &["status", "--porcelain"]), "");
  assert_eq!(git(&page.fixture.task_root, &["rev-parse", "HEAD"]), task);
  assert_eq!(git(page.root(), &["rev-parse", "HEAD"]), main);
  let db = ProjectDb::open(page.root()).expect("db");
  let operation = db.local_operation(page.run()).expect("journal").expect("exists");
  assert_eq!(operation.phase(), LocalPhase::Paused);
  assert!(operation.preservation().expect("recovery").retained);
  assert!(db.run_merge(page.run()).expect("merge").is_none());
  let mut session = page.fixture.open();
  session.reconcile().expect("resume after abort");
  session
    .cancel_preservation()
    .expect("retained originals can still be restored");
  assert_eq!(
    fs::read(page.root().join("local.txt")).expect("originals"),
    b"retained local changes\n"
  );
  assert_eq!(git(page.root(), &["rev-parse", "HEAD"]), main);
}

#[test]
fn local_integration_unresolved_task_abort_still_requires_acknowledgement_and_clean_editor() {
  let mut page = conflict();
  let before = task_git(&page).snapshot().expect("owned conflict");
  page.click("local-review-abort");
  page.click("local-confirm");
  assert_eq!(task_git(&page).snapshot().expect("still unresolved"), before);
  page.type_into("local-result-editor", "unsaved result candidate");
  assert!(page.has("local-unsaved"));
  page.click("local-confirm-scope");
  page.click("local-confirm");
  assert_eq!(task_git(&page).snapshot().expect("no abort while dirty"), before);
  assert_eq!(page.value("local-result-editor"), "unsaved result candidate");
  assert!(!page.has("local-working"), "disabled confirmation cannot start work");
}

#[test]
fn local_integration_unresolved_task_abort_refuses_synthetic_confirmation() {
  let mut page = conflict();
  page.window.link.input_gate().bind_window_control(true);
  let before = task_git(&page).snapshot().expect("conflict");
  page.click("local-review-abort");
  assert!(page.window.link.input_gate().protecting());
  page.click("local-confirm-scope");
  with_synthetic_input(|| page.click("local-confirm"));
  page.until("protected abort refusal", |window| {
    window.texts("local-work-error").is_some()
  });
  assert_eq!(task_git(&page).snapshot().expect("unchanged"), before);
}

#[test]
fn local_integration_unresolved_task_abort_refuses_foreign_edits() {
  let mut page = conflict();
  page.click("local-review-abort");
  page.click("local-confirm-scope");
  fs::write(page.fixture.task_root.join("foreign.txt"), b"new foreign work\n").expect("external edit");
  let before = task_git(&page).snapshot().expect("foreign version");
  page.click("local-confirm");
  page.until("owned version refusal", |window| {
    window.texts("local-work-error").is_some()
  });
  assert_eq!(task_git(&page).snapshot().expect("retained foreign state"), before);
  assert_eq!(
    fs::read(page.fixture.task_root.join("foreign.txt")).expect("retained"),
    b"new foreign work\n"
  );
}
