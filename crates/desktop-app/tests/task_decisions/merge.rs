//! Ready for review with Merge into main: opening the local checkout review (also for a task that will conflict),
//! unavailable without a branch to merge into, merged; and what git tells of the work without Orchester: merged by
//! a person, or nothing to merge, and current external Git evidence in place of Ready.

use std::process;

use orchester_core::{
  run::Run,
  store::{RunStore, TaskStore},
};
use orchester_store::ProjectDb;

use super::{Page, key_of};

/// The latest run of the task seeded in `state`.
pub(super) fn latest_run(page: &Page, state: &str) -> Run {
  let key = key_of(page, state);
  let db = ProjectDb::open(&page.repo).expect("the project database");
  let task = db
    .tasks()
    .expect("the tasks")
    .into_iter()
    .find(|task| task.key().to_string() == key)
    .expect("the seeded task");
  db.runs_of_task(task.id())
    .expect("its runs")
    .pop()
    .expect("a run")
    .expect("a readable run")
}

/// `git <args>` in the seeded repository, which must succeed.
pub(super) fn git(page: &Page, args: &[&str]) {
  let output = process::Command::new("git")
    .arg("-C")
    .arg(&page.repo)
    .args(args)
    .output()
    .expect("run git");
  assert!(
    output.status.success(),
    "git {args:?}: {}",
    String::from_utf8_lossy(&output.stderr)
  );
}

#[test]
fn decision_ready_for_review_opens_local_integration_before_any_merge() {
  let mut page = Page::seeded("decisions");
  page.open("ready for review, merges cleanly", "task-merge");
  assert_eq!(page.text("task-notice-title"), "Ready for your review");
  assert!(page.locked("task-review-pull-request"));
  assert_eq!(
    page.text("task-review-pull-request-reason"),
    "Review PR needs a pull request: create one for this run first."
  );
  assert!(page.locked("task-ask-agent"), "Ask agent waits for the composer");
  page.until_says("task-ready-changes-figure", "1 file");
  assert!(page.has("task-ready-check-review"), "Review verified the work");
  assert!(page.has("task-run-details"));
  page.click("task-merge");
  page.window.until("local checkout review", |window| {
    window.tree.get_element_by_id("task-local-integration").is_some()
  });
  assert!(
    page
      .window
      .navigation()
      .route_now()
      .path()
      .ends_with("/local-integration")
  );
  assert!(!page.has("local-confirm"), "no merge approval before a fresh Check");
}

#[test]
fn decision_merge_opens_local_review_for_a_task_that_will_conflict() {
  let mut page = Page::seeded("decisions");
  page.open("ready for review, conflicts with main", "task-merge");
  page.click("task-merge");
  page.window.until("local checkout review", |window| {
    window.tree.get_element_by_id("task-local-integration").is_some()
  });
  assert!(
    page
      .window
      .navigation()
      .route_now()
      .path()
      .ends_with("/local-integration")
  );
  assert!(
    !page.has("local-result-editor"),
    "sync has not begun from an entry click"
  );
}

#[test]
fn decision_merge_is_unavailable_without_a_branch_and_merged_says_where() {
  let mut page = Page::seeded("decisions");
  page.open("ready for review, no branch to merge into", "task-merge");
  assert!(page.locked("task-merge"));
  assert_eq!(
    page.text("task-merge-reason"),
    "Merge needs the branch your checkout had when the run started, which this run didn't record."
  );
  let mut page = Page::seeded("task-states");
  page.open("merged", "task-notice");
  assert_eq!(page.text("task-notice-title"), "Merged into main");
  assert!(page.has("task-stages"));
  assert!(!page.has("task-merge"));
  assert!(!page.has("task-review-pull-request"));
}

#[test]
fn decision_ready_work_merged_with_git_reads_merged_without_a_merge_action() {
  let mut page = Page::seeded("decisions");
  let run = latest_run(&page, "ready for review, merges cleanly");
  git(
    &page,
    &[
      "merge",
      "--quiet",
      "--no-ff",
      "-m",
      "merged by hand",
      run.branch().as_str(),
    ],
  );
  page.open("ready for review, merges cleanly", "task-notice");
  assert_eq!(page.text("task-notice-title"), "Merged");
  assert!(page.text("task-notice").contains("merged without Orchester"));
  assert!(!page.has("task-merge"), "nothing waits to be merged");
}

#[test]
fn decision_a_run_that_committed_nothing_has_nothing_to_merge() {
  let mut page = Page::seeded("decisions");
  let run = latest_run(&page, "ready for review, merges cleanly");
  let branch = format!("refs/heads/{}", run.branch());
  git(&page, &["update-ref", &branch, run.base().as_str()]);
  page.open("ready for review, merges cleanly", "task-notice");
  assert_eq!(page.text("task-notice-title"), "Nothing to merge");
  assert!(!page.has("task-merge"), "there is no work to merge");
}

#[test]
fn decision_squashed_external_integration_replaces_ready_without_a_historical_merge_record() {
  use orchester_core::{
    integration::local::LocalIntegrationStore,
    store::{MergeStore, RunStore, TaskStore},
  };
  use orchester_store::ProjectDb;

  let mut page = Page::seeded("decisions");
  let db = ProjectDb::open(&page.repo).expect("project");
  let key = key_of(&page, "ready for review, merges cleanly");
  let task = db
    .tasks()
    .expect("tasks")
    .into_iter()
    .find(|task| task.key().to_string() == key)
    .expect("task");
  let run = db
    .runs_of_task(task.id())
    .expect("runs")
    .into_iter()
    .filter_map(Result::ok)
    .max_by_key(|run| run.number().get())
    .expect("run");
  // A squash merge: the work is in main without its history, which only the local integration's evidence finds;
  // a complete merge reads as merged without Orchester instead.
  git(&page, &["merge", "--squash", run.branch().as_str()]);
  git(&page, &["commit", "-m", "External QA integration"]);
  page.open("ready for review, merges cleanly", "task-external-integration");
  assert!(!page.has("task-merge"));
  assert!(!page.has("task-notice-title"));
  assert!(
    page
      .text("task-external-integration")
      .contains("Integration result needs reconciliation")
  );
  assert!(db.run_merge(run.id()).expect("record").is_none());
  assert!(db.local_operation(run.id()).expect("operation").is_none());
  page.click("task-inspect-external-integration");
  page.window.until("current external Git evidence", |window| {
    window.tree.get_element_by_id("local-external-integration").is_some()
  });
  assert!(!page.has("local-sync"));
  assert!(!page.has("local-confirm"));
}
