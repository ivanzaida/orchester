//! *Changed since your review* on a task's review page (ORC-798, `Review - v2` 05 `e6KEO`), in the app's real window on
//! the QA seed's `review-changed` scenario: a run whose branch moved two commits after the head reviewed, the first of
//! which rewrote a line a comment is on. The status counts the commits and what they change, the strip under it says
//! how many commits and comments, its switch chooses between the whole diff and the diff since the review and is kept
//! per task while the window lives, and a branch rewritten after the review says why there is no diff since it. Needs
//! `git`, as the seed does.
//! `cargo test -p orchester-app --test review_since review_since`.

#[path = "../examples/seed/kontur/mod.rs"]
#[allow(dead_code, reason = "the seed's helpers serve every scenario; these tests seed one")]
mod kontur;
#[path = "review_since/page.rs"]
#[allow(dead_code, reason = "shared by two test binaries, each of which uses a part")]
mod page;
mod support;

use std::path::Path;

use orchester_app::routes::{ProjectPage, Route};

use crate::page::{GIT_WORK, MIGRATION, Page, SENDER, git};

/// Squashes the run's branch onto `main` in one new commit, as a rebase would: the head the person reviewed is no
/// longer in its history.
fn rewrite_run_branch(repo: &Path) {
  let branches = git(repo, &["for-each-ref", "--format=%(refname:short)", "refs/heads"]);
  let branch = branches.lines().find(|name| *name != "main").expect("the run's branch");
  let base = git(repo, &["merge-base", branch, "main"]);
  let tree = git(repo, &["rev-parse", &format!("{branch}^{{tree}}")]);
  let squashed = git(repo, &["commit-tree", &tree, "-p", &base, "-m", "Squash the run"]);
  git(repo, &["update-ref", &format!("refs/heads/{branch}"), &squashed]);
}

#[test]
fn review_since_counts_the_commits_and_the_outdated_comments_and_shows_what_changed_since() {
  let mut page = Page::new(|_| {});
  page.open();
  assert_eq!(page.window.text("review-status-text"), "Changed since your review");
  let status = page.status();
  assert!(status.contains("2 commits after "), "{status}");
  assert!(status.contains("into main"), "{status}");
  let strip = page.window.text("review-since");
  assert!(strip.contains("2 commits since you reviewed at "), "{strip}");
  assert!(
    strip.contains("1 of your comments is on a line they changed."),
    "{strip}"
  );
  assert!(
    strip.contains("All changes") && strip.contains("Since your review"),
    "{strip}"
  );
  // Since your review is shown first: only what the two commits changed.
  page.until_diff("the diff since the review", |diff| diff.contains("under 30 s"));
  let diff = page.window.text("review-diff");
  assert!(!diff.contains(SENDER), "the commits did not touch it: {diff}");
  assert_eq!(
    page.window.text("review-file-0-path"),
    "notes/retry.md",
    "each file's head names the file whose lines are under it"
  );
  page
    .window
    .until_within(GIT_WORK, "the size of the diff shown", |window| {
      window.text("review-status-meta").contains(" files, +")
    });
}

#[test]
fn review_since_switch_shows_all_changes_and_keeps_the_choice_for_the_task() {
  let mut page = Page::new(|_| {});
  page.open();
  page.until_diff("the diff since the review", |diff| !diff.contains(SENDER));
  page.window.press("review-since-all");
  page.until_diff("every change of the run", |diff| diff.contains(SENDER));
  let status = page.status();
  assert!(
    status.contains("2 commits after ") && status.contains("5 files, "),
    "{status}"
  );
  assert_eq!(
    page.window.text("review-file-0-path"),
    MIGRATION,
    "the heads name the files of the whole diff again"
  );
  // Leaving the page and coming back keeps the choice while the window lives.
  page
    .window
    .navigation()
    .go(Route::Project(page.project, ProjectPage::Review));
  page.window.until_within(GIT_WORK, "the Review list", |window| {
    window.texts("review-since").is_none()
  });
  page.open();
  page.until_diff("every change of the run", |diff| diff.contains(SENDER));
  page.window.press("review-since-only");
  page.until_diff("the diff since the review", |diff| !diff.contains(SENDER));
}

#[test]
fn review_since_a_rewritten_branch_says_why_and_all_changes_still_shows_the_diff() {
  let mut page = Page::new(rewrite_run_branch);
  page.open();
  page.window.until_within(GIT_WORK, "the reason", |window| {
    window.texts("review-since-failed").is_some()
  });
  let reason = page.window.text("review-since-failed");
  assert!(reason.contains("rewritten after the head you reviewed"), "{reason}");
  assert!(
    page.window.texts("review-diff").is_none(),
    "the reason stands in the diff's place"
  );
  page.window.press("review-since-all");
  page.until_diff("every change of the run", |diff| diff.contains(SENDER));
}

#[test]
fn review_since_shows_no_strip_and_the_whole_diff_while_the_branch_has_no_commits_after_the_review() {
  let mut page = Page::of("review-page", |_| {});
  page.open();
  assert_eq!(page.window.text("review-status-text"), "Waiting for your review");
  assert!(
    page.window.texts("review-since").is_none(),
    "nothing changed since the review"
  );
  page.until_diff("the whole diff", |diff| diff.contains(SENDER));
}
