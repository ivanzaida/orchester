//! Ready for review with Merge into main: merging beside the person's uncommitted work (also past a local
//! integration paused at its review), refused at a dirty path the run changes and at a conflict with Local
//! integration offered, unavailable without a branch to merge into, merged; and what git tells of the work without
//! Orchester: merged by a person, and current external Git evidence in place of Ready. A run with nothing to merge is
//! [`super::completion`]'s.

use std::{fs, process};

use orchester_core::{
  integration::local::{LocalIntegrationStore, LocalPhase},
  run::Run,
  store::{MergeStore, RunStore, TaskStore},
};
use orchester_store::ProjectDb;
use orchester_workspace::tasks::local_integration::LocalIntegration;

use super::{Page, checkout_page, key_of};

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

/// `git <args>` in the seeded repository, which must succeed; what it printed.
fn git_out(page: &Page, args: &[&str]) -> String {
  let output = process::Command::new("git")
    .arg("-C")
    .arg(&page.repo)
    .args(args)
    .output()
    .expect("run git");
  assert!(output.status.success(), "git {args:?}");
  String::from_utf8(output.stdout).expect("text").trim().to_owned()
}

/// The files `run` changed on its branch.
fn changed_by(page: &Page, run: &Run) -> Vec<String> {
  git_out(
    page,
    &["diff", "--name-only", run.base().as_str(), run.branch().as_str()],
  )
  .lines()
  .map(str::to_owned)
  .collect()
}

/// Writes the person's work into the checkout: `modified` changed and not staged (or, where the checkout has no such
/// file, written there untracked), and an untracked file of its own; their paths and bytes.
fn person_work(page: &Page, modified: &str) -> Vec<(String, Vec<u8>)> {
  let path = page.repo.join(modified);
  let mut edited = if path.exists() {
    fs::read(&path).expect("the tracked file")
  } else {
    fs::create_dir_all(path.parent().expect("a file in the repository")).expect("its folder");
    Vec::new()
  };
  edited.extend_from_slice(b"\nThe person's edit, not committed.\n");
  fs::write(page.repo.join(modified), &edited).expect("edit");
  fs::write(page.repo.join("handover-notes.md"), "Untracked notes.\n").expect("untracked");
  [modified, "handover-notes.md"]
    .into_iter()
    .map(|path| (path.to_owned(), fs::read(page.repo.join(path)).expect("bytes")))
    .collect()
}

fn unchanged(page: &Page, work: &[(String, Vec<u8>)]) {
  for (path, bytes) in work {
    assert_eq!(
      &fs::read(page.repo.join(path)).expect("bytes"),
      bytes,
      "{path} is the person's"
    );
  }
}

/// The commit `run`'s branch is at; its merge removes the branch.
fn tip(page: &Page, run: &Run) -> String {
  git_out(page, &["rev-parse", run.branch().as_str()])
}

/// Whether Merged into main stands for `run`, whose branch was at `tip`: that commit is in the checkout's branch and
/// the merge is recorded.
fn merged(page: &Page, (run, tip): (&Run, &str)) -> bool {
  let ancestor = process::Command::new("git")
    .arg("-C")
    .arg(&page.repo)
    .args(["merge-base", "--is-ancestor", tip, "HEAD"])
    .status()
    .expect("run git")
    .success();
  let recorded = ProjectDb::open(&page.repo)
    .expect("the project database")
    .run_merge(run.id())
    .expect("the merge record")
    .is_some();
  ancestor && recorded
}

#[test]
fn decision_merge_into_main_merges_beside_unrelated_uncommitted_work() {
  let _checkout = checkout_page();
  let mut page = Page::seeded("decisions");
  let run = latest_run(&page, "ready for review, merges cleanly");
  assert!(!changed_by(&page, &run).contains(&"README.md".to_owned()));
  let work = person_work(&page, "README.md");
  let tip = tip(&page, &run);
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
  assert!(page.has("task-facts"), "the run's facts beside the card");
  page.merge_until(("task-merge", "task-notice-title"), "Merged into main");
  assert!(page.text("task-notice").contains("main got merge commit"));
  assert!(
    !page.has("task-local-integration"),
    "no detour through Local integration"
  );
  assert!(merged(&page, (&run, &tip)));
  unchanged(&page, &work);
  assert_eq!(
    git_out(
      &page,
      &["status", "--porcelain", "--", "README.md", "handover-notes.md"]
    ),
    "M README.md\n?? handover-notes.md",
    "git_out trims the first line's leading space"
  );
}

#[test]
fn decision_merge_merges_a_run_whose_local_integration_paused_at_review() {
  let _checkout = checkout_page();
  let mut page = Page::seeded("decisions");
  let run = latest_run(&page, "ready for review, merges cleanly");
  let work = person_work(&page, "README.md");
  let tip = tip(&page, &run);
  let mut review = LocalIntegration::open(&page.repo, run.id()).expect("the review of the checkout");
  review.pause().expect("paused at its review");
  drop(review);
  let paused = ProjectDb::open(&page.repo)
    .expect("the project database")
    .local_operation(run.id())
    .expect("read")
    .expect("the paused review");
  assert_eq!(paused.phase(), LocalPhase::Paused);
  page.open("ready for review, merges cleanly", "task-merge");
  page.merge_until(("task-merge", "task-notice-title"), "Merged into main");
  assert!(merged(&page, (&run, &tip)));
  unchanged(&page, &work);
}

#[test]
fn decision_merge_at_a_dirty_path_says_why_and_offers_local_integration() {
  let _checkout = checkout_page();
  let mut page = Page::seeded("decisions");
  let run = latest_run(&page, "ready for review, merges cleanly");
  let touched = changed_by(&page, &run)
    .into_iter()
    .next()
    .expect("a file the run changed");
  let work = person_work(&page, &touched);
  let tip = tip(&page, &run);
  let head = git_out(&page, &["rev-parse", "HEAD"]);
  page.open("ready for review, merges cleanly", "task-merge");
  page.merge_until(("task-merge", "task-notice-title"), "Not merged into main");
  assert!(
    page
      .text("task-notice")
      .contains("uncommitted changes the merge would overwrite"),
    "{}",
    page.text("task-notice")
  );
  assert_eq!(git_out(&page, &["rev-parse", "HEAD"]), head);
  unchanged(&page, &work);
  assert!(!merged(&page, (&run, &tip)));
  assert!(page.has("task-merge"), "Merge into main stays for another try");
  page.click("task-open-local-integration");
  page.window.until("Local integration", |window| {
    window.tree.get_element_by_id("task-local-integration").is_some()
  });
  let path = page.window.navigation().route_now().path();
  assert!(path.ends_with("/changes?view=local-integration"), "{path}");
}

#[test]
fn decision_merge_conflict_lists_the_files_and_offers_local_integration() {
  let _checkout = checkout_page();
  let mut page = Page::seeded("decisions");
  let run = latest_run(&page, "ready for review, conflicts with main");
  let tip = tip(&page, &run);
  let head = git_out(&page, &["rev-parse", "HEAD"]);
  page.open("ready for review, conflicts with main", "task-merge");
  page.merge_until(("task-merge", "task-notice-title"), "Not merged into main");
  page.until_says("task-merge-conflicts", "src/webhooks/sender.rs");
  assert!(page.text("task-merge-conflicts").contains("src/webhooks/settings.rs"));
  assert!(page.has("task-open-local-integration"));
  assert_eq!(git_out(&page, &["rev-parse", "HEAD"]), head, "the merge was undone");
  assert!(!merged(&page, (&run, &tip)));
}

#[test]
fn decision_merge_is_unavailable_without_a_branch_and_merged_says_where() {
  let _checkout = checkout_page();
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
  assert!(page.has("task-steps"), "the run's rail under the card");
  assert!(!page.has("task-merge"));
  assert!(!page.has("task-review-pull-request"));
}

#[test]
fn decision_ready_work_merged_with_git_reads_merged_without_a_merge_action() {
  let _checkout = checkout_page();
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
  page.open("ready for review, merges cleanly", "task-event");
  let event = page.text("task-event-text");
  assert!(event.starts_with("Merged. "), "{event}");
  assert!(event.contains("merged without Orchester"), "{event}");
  assert!(!page.has("task-merge"), "nothing waits to be merged");
}

#[test]
fn decision_squashed_external_integration_replaces_ready_without_a_historical_merge_record() {
  let _checkout = checkout_page();
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
