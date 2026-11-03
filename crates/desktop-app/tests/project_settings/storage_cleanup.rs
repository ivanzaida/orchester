//! Cleanup uses real git worktrees and a confirmation; only ended runs are candidates.
use std::fs;

use orchester_app::routes::{ProjectPage, Route};
use orchester_core::{store::RunStore, time::Timestamp};

use super::{harness::Settings, run_fixture};
use crate::support::project_id;

#[test]
fn storage_cleanup_confirmation_lists_only_ended_worktrees_and_removes_after_confirming() {
  let mut page = Settings::new(false);
  for args in [
    vec!["init", "-b", "main"],
    vec!["config", "user.name", "Storage test"],
    vec!["config", "user.email", "storage@example.test"],
    vec!["commit", "--allow-empty", "-m", "Base"],
  ] {
    assert!(
      std::process::Command::new("git")
        .args(args)
        .current_dir(&page.repo)
        .output()
        .expect("git")
        .status
        .success()
    );
  }
  let mut db = page.db();
  let run = run_fixture::started_run(&mut db);
  let mut stored = db.run(run).expect("read").expect("run");
  let events = stored.cancel(Timestamp::now()).expect("cancel");
  db.append_events(&stored, &events).expect("ended");
  let path = orchester_workspace::tasks::changes::run_worktree(&page.repo, run);
  fs::create_dir_all(&path).expect("forgotten folder");
  fs::write(path.join("keep.txt"), "uncommitted data").expect("files");
  page
    .window
    .navigation()
    .go(Route::Project(project_id(), ProjectPage::Storage));
  page.until("measured", |p| {
    p.exists("storage-cleanup-hint") && p.text("storage-cleanup-hint").contains("1 ended")
  });
  page.click("storage-clean-up");
  page.until("cleanup confirmation", |p| p.exists("storage-confirm-title"));
  assert!(page.text("storage-confirm").contains("Their worktrees go"));
  assert!(path.exists(), "asking doesn't delete anything");
  page.click("storage-confirm-cancel");
  assert!(path.exists());
  page.click("storage-clean-up");
  page.until("cleanup confirmation again", |p| p.exists("storage-confirm-title"));
  page.click("storage-confirm-remove");
  page.until("folder removed", |_| !path.exists());
}
