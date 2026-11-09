//! *Since your review*: the run's diff from the head the person reviewed to where its branch is now, in the shape of
//! the page's whole diff; no diff, with why, when no head was reviewed or the branch was rewritten after it.

use orchester_core::review::ThreadState;
use orchester_exec::programs::Programs;
use orchester_workspace::tasks::{
  changes::Checkpointed,
  review_threads::{
    self,
    read::review_of,
    since::{SinceReviewError, diff_since_review},
  },
};

use super::support::{Repo, at, body, git};

fn since(repo: &Repo, reviewed: Option<&orchester_core::revision::Revision>) -> Result<Checkpointed, SinceReviewError> {
  let programs = Programs::processes();
  let review = review_of(repo.root(), &programs, repo.run).expect("review");
  diff_since_review((repo.root(), &programs), (reviewed, review.head.as_ref()))
}

#[test]
fn review_threads_since_review_is_empty_while_the_branch_is_at_the_reviewed_head() {
  let repo = Repo::new();
  let Ok(Checkpointed::Read { head, changes }) = since(&repo, Some(&repo.head)) else {
    panic!("the diff is read");
  };
  assert_eq!(head, repo.head);
  assert_eq!(changes.from, repo.head);
  assert!(changes.diff.files().is_empty());
}

#[test]
fn review_threads_since_review_shows_the_new_commits_and_outdates_the_thread_on_a_line_they_changed() {
  let repo = Repo::new();
  let (anchor, head) = repo.anchor(1, 1);
  review_threads::comment(repo.root(), repo.run, (anchor, head), &body("Rename one."), at(20)).expect("kept");
  let reviewed = repo.head.clone();
  let newer = repo.commit("fn first() {}\nfn two() {}\nfn three() {}\nfn four() {}\n");

  let Ok(Checkpointed::Read { head, changes }) = since(&repo, Some(&reviewed)) else {
    panic!("the diff is read");
  };
  assert_eq!(head, newer);
  assert_eq!(changes.from, reviewed);
  let files = changes.diff.files();
  assert_eq!(files.len(), 1);
  assert_eq!(files[0].path(), "src/lib.rs");

  let review = review_of(repo.root(), &Programs::processes(), repo.run).expect("review");
  assert_eq!(review.threads[0].state, ThreadState::Outdated { commit: newer });
}

#[test]
fn review_threads_since_review_says_why_when_nothing_was_reviewed_or_the_branch_was_rewritten() {
  let repo = Repo::new();
  let reviewed = repo.head.clone();
  assert!(matches!(since(&repo, None), Err(SinceReviewError::NotReviewed)));

  git(
    &repo.worktree,
    &["commit", "--amend", "-m", "rewritten", "--allow-empty"],
  );
  let rewritten = git(&repo.worktree, &["rev-parse", "HEAD"]);
  assert_ne!(rewritten, reviewed.as_str());
  let Err(SinceReviewError::Rewritten { reviewed: said, head }) = since(&repo, Some(&reviewed)) else {
    panic!("the reviewed head is no ancestor of the rewritten branch");
  };
  assert_eq!(said, reviewed);
  assert_eq!(head.as_str(), rewritten);
}
