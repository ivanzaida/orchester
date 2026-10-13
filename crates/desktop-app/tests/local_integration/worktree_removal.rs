//! Private UI harness behavior; protected human decisions and visual parity remain separate verification.
use std::fs;

use orchester_app::routes::{ProjectPage, Route, TaskView};
use orchester_core::{
  integration::local::{LocalIntegrationStore, LocalPath},
  store::{RunStore, WorktreeStore},
};
use orchester_store::ProjectDb;

use super::{
  domain_support::{Fixture, git},
  integration_support::Page,
};

#[test]
fn local_integration_worktree_removal_refusal_keeps_inline_confirmation_and_recovery_reachable() {
  let fixture = Fixture::new(false);
  fs::write(fixture.root().join("local.txt"), b"completed commit\n").expect("edit");
  git(fixture.root(), &["add", "local.txt"]);
  let path = LocalPath::new("local.txt").expect("path");
  let mut session = fixture.open();
  let tree = session.selected_commit_tree(std::slice::from_ref(&path)).expect("tree");
  session.commit(&[path], &tree, "local commit").expect("commit");
  drop(session);
  let mut page = Page::new(fixture);
  page.ready("local-review-cancel");
  let mut summary = page.route.clone();
  let Route::Project(_, ProjectPage::Task(_, view)) = &mut summary else {
    panic!("task route");
  };
  *view = TaskView::Summary;
  page.window.navigation().go(summary);
  page.ready("task-remove-worktree");
  let db = ProjectDb::open(page.root()).expect("db");
  let run = db.run(page.run()).expect("read").expect("run");
  let number = run.number().get();
  let saved = db.local_operation(page.run()).expect("read").expect("operation");
  assert!(saved.reserves_checkout());
  page.click("task-remove-worktree");
  page.ready(&format!("run-remove-start-{number}"));
  page.click(&format!("run-remove-start-{number}"));
  page.until("reserved worktree refusal", |window| {
    window
      .texts(&format!("run-remove-confirm-{number}"))
      .is_some_and(|texts| {
        texts
          .iter()
          .any(|text| text.contains("This worktree is reserved by local integration"))
      })
  });
  assert!(page.fixture.task_root.is_dir());
  assert_eq!(db.worktree_removal(page.run()).expect("read"), None);
  assert_eq!(db.local_operation(page.run()).expect("read"), Some(saved));
  page.window.navigation().go(page.route.clone());
  page.ready("local-review-cancel");
}
