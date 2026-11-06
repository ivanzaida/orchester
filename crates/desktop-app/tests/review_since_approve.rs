//! *Approve and merge* on a task's review whose branch has commits after the head reviewed (ORC-798): nothing holds
//! it back, and it merges the head shown. Its own test binary: a merge waits for the process the run left to stop, and
//! another test's git programs running in the same process would make it refuse.
//! `cargo test -p orchester-app --test review_since_approve review_since`.

#[path = "../examples/seed/kontur/mod.rs"]
#[allow(dead_code, reason = "the seed's helpers serve every scenario; these tests seed one")]
mod kontur;
#[path = "review_since/page.rs"]
#[allow(dead_code, reason = "shared by two test binaries, each of which uses a part")]
mod page;
mod support;

use std::process::Command;

use crate::page::{GIT_WORK, Page, git};

#[test]
fn review_since_approve_on_a_changed_branch_merges_at_the_head_shown() {
  let mut page = Page::new(|_| {});
  let repo = page.repo.clone();
  let branches = git(&repo, &["for-each-ref", "--format=%(refname:short)", "refs/heads"]);
  let branch = branches.lines().find(|name| *name != "main").expect("the run's branch");
  let tip = git(&repo, &["rev-parse", branch]);
  page.open();
  assert!(
    page.window.texts("review-approve-reason").is_none(),
    "Approve is not held back"
  );
  page.window.press("review-approve");
  page.window.until_within(GIT_WORK, "the merge", |window| {
    window.texts("review-not-done").is_some()
      || window
        .texts("review-status-meta")
        .is_some_and(|meta| meta.concat().starts_with("approved by you "))
  });
  assert!(
    page.window.texts("review-not-done").is_none(),
    "{}",
    page.window.text("review-not-done")
  );
  let merged = Command::new("git")
    .arg("-C")
    .arg(&repo)
    .args(["merge-base", "--is-ancestor", &tip, "main"])
    .status()
    .expect("git runs");
  assert!(merged.success(), "main holds the head that was shown: {tip}");
}
