//! Cleanup uses real git worktrees and a confirmation; only ended runs are candidates.
use std::fs;

use lurq::app::{
  events::MouseButton,
  synthetic_input::{SyntheticInput, apply},
};
use orchester_app::routes::{ProjectPage, Route};
use orchester_core::{
  integration::{NotRemoved, WORKTREES_DIR},
  store::{RunStore, WorktreeStore},
  time::Timestamp,
};

use super::{harness::Settings, run_fixture};
use crate::support::{
  link_press::{Presser, press_control},
  project_id,
};

#[test]
fn storage_cleanup_confirmation_lists_only_ended_worktrees_and_removes_after_confirming() {
  let mut page = project();
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
  assert!(page.text("storage-confirm").contains("A person decides"));
  press_control(
    &mut page.window,
    "storage-confirm-remove",
    (Presser::Synthetic, MouseButton::Left),
    (false, false),
  );
  assert!(path.exists(), "a synthetic click cannot discard files");
  assert!(page.exists("storage-confirm-title"));
  apply(&mut page.window.tree, &SyntheticInput::key_down("Enter"));
  page.window.pass();
  assert!(path.exists(), "synthetic Enter cannot discard files");
  assert!(page.exists("storage-confirm-title"));
  page.click("storage-confirm-remove");
  page.until("folder removed", |_| !path.exists());
}

#[test]
fn storage_cleanup_retry_keeps_a_failed_step_after_the_main_worktree_is_removed() {
  let mut page = project();
  let mut db = page.db();
  let run = run_fixture::started_run(&mut db);
  let mut stored = db.run(run).expect("read").expect("run");
  let execution = stored.current_attempt().expect("attempt").executions()[0].id();
  let events = stored.cancel(Timestamp::now()).expect("cancel");
  db.append_events(&stored, &events).expect("ended");
  let step = page.repo.join(WORKTREES_DIR).join(execution.to_string());
  fs::create_dir_all(&step).expect("leftover step folder");
  fs::write(step.join("keep.txt"), "previously confirmed files").expect("files");
  // A partial cleanup has removed the main worktree and left a step folder behind.
  db.save_worktree_removal(run, Timestamp::now())
    .expect("confirmed main removal");
  db.save_worktree_not_removed(
    run,
    &NotRemoved {
      path: step.to_string_lossy().replace('\\', "/"),
      reason: Some("held by a process".into()),
    },
    Timestamp::now(),
  )
  .expect("record failed step");
  page
    .window
    .navigation()
    .go(Route::Project(project_id(), ProjectPage::Storage));
  let retry = format!("storage-remove-{}", step.display());
  page.until("retry for failed step", |p| {
    p.exists(&retry) && p.text("storage-cleanup-hint").contains("1 ended")
  });
  assert_eq!(page.text(&retry), "Try again");
  page.click(&retry);
  page.until("retry confirmation", |p| p.exists("storage-confirm-title"));
  assert!(page.text("storage-confirm").contains(&execution.to_string()));
  assert!(!page.text("storage-confirm").contains("A person decides"));
  apply(&mut page.window.tree, &SyntheticInput::key_down("Enter"));
  page.window.pass();
  page.until("step folder removed", |_| !step.exists());
  page.until("empty inventory", |p| p.exists("storage-empty"));
}

fn project() -> Settings {
  let page = Settings::new(false);
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
  page
}
