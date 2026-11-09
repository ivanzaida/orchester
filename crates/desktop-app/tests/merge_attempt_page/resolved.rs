//! A run whose attempt stopped on conflicts and whose branch holds its base branch since (an agent or a person merged
//! it in; ORC-920): the page offers the merge, not the conflicts, and no way to resolve what no longer conflicts.

use std::process::Command;

use super::{Page, merged::run_of};

/// `git <args>` in the seeded repository, which must succeed; what it printed.
fn git(page: &Page, args: &[&str]) -> String {
  let output = Command::new("git")
    .current_dir(&page.repo)
    .args(["-c", "user.name=Test", "-c", "user.email=test@example.com"])
    .args(args)
    .output()
    .expect("git runs");
  assert!(output.status.success(), "git {args:?}: {output:?}");
  String::from_utf8(output.stdout).expect("text").trim().to_owned()
}

/// Makes `branch` hold `into`'s head, as a merge of it into the branch does, without touching any checkout.
fn merge_in(page: &Page, branch: &str, into: &str) {
  let (tip, base) = (git(page, &["rev-parse", branch]), git(page, &["rev-parse", into]));
  let tree = git(page, &["rev-parse", &format!("{branch}^{{tree}}")]);
  let merge = git(
    page,
    &[
      "commit-tree",
      &tree,
      "-p",
      &tip,
      "-p",
      &base,
      "-m",
      "Merge the base branch",
    ],
  );
  git(page, &["update-ref", &format!("refs/heads/{branch}"), &merge]);
}

#[test]
fn merge_attempt_page_conflicts_the_branch_already_resolved_offer_the_merge_not_the_conflicts() {
  let mut page = Page::new();
  let run = run_of(&page, "Per-endpoint secrets");
  merge_in(&page, run.branch().as_str(), "main");
  page.open("Per-endpoint secrets");
  assert_eq!(
    page.window.text("merge-attempt-status-text"),
    "Ready to merge into main"
  );
  for gone in [
    "merge-attempt-resolve-in-editor",
    "merge-attempt-ask-agent",
    "merge-attempt-conflicting-files",
  ] {
    assert!(!page.shows(gone), "{gone}: nothing conflicts any more");
  }
  assert!(page.shows("merge-attempt-try-again"), "the merge is offered");
  let words = page.window.text("merge-attempt-explanation");
  assert!(words.contains("holds main now"), "{words}");
  assert_eq!(
    page.window.text("merge-attempt-bar-title"),
    "main is already in the run's branch"
  );
}
