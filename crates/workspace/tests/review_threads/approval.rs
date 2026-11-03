//! *Approve and merge* merges only at the head the person reviewed: a branch that moved since merges nothing and asks
//! for review again, and nothing is approved while Implement works on comments sent to it.

use orchester_core::{run::ReviewNote, store::MergeStore};
use orchester_exec::programs::Programs;
use orchester_workspace::tasks::review_threads::{
  self, ReviewRefusal,
  approve::{Approved, approve_and_merge},
  send,
};

use super::support::{Repo, at, body, git};

#[test]
fn review_threads_approve_merges_only_at_the_reviewed_head() {
  let repo = Repo::new();
  let reviewed = repo.head.clone();
  let moved = repo.commit("fn one() {}\nfn two() {}\nfn three() {}\nfn four() {}\n");
  let refused = approve_and_merge(repo.root(), &Programs::processes(), (repo.run, &reviewed), at(20))
    .map(|_| ())
    .expect_err("moved");
  assert_eq!(refused, ReviewRefusal::ChangedSinceReview);
  assert_eq!(refused.message().key, "review.refused.changedSinceReview");
  assert!(repo.db().run_merge(repo.run).expect("read").is_none(), "nothing merged");
  let main = git(repo.root(), &["rev-parse", "main"]);

  let approved =
    approve_and_merge(repo.root(), &Programs::processes(), (repo.run, &moved), at(21)).expect("merged at the head");
  let Approved::Merged(merged) = approved else {
    panic!("the task page's merge, not the queue's");
  };
  assert_eq!(merged.merge.run, repo.run);
  assert_ne!(git(repo.root(), &["rev-parse", "main"]), main);
  assert!(repo.db().run_merge(repo.run).expect("read").is_some());
  assert_eq!(
    approve_and_merge(repo.root(), &Programs::processes(), (repo.run, &moved), at(22)).map(|_| ()),
    Err(ReviewRefusal::NotWaiting),
    "a merged run waits for no review"
  );
}

#[test]
fn review_threads_approve_is_refused_while_implement_works_on_sent_comments() {
  let repo = Repo::new();
  review_threads::comment(repo.root(), repo.run, repo.anchor(1, 1), &body("Rename one."), at(20)).expect("kept");
  let sending = send::begin(repo.root(), &Programs::processes(), repo.run).expect("sending");
  assert_eq!(
    approve_and_merge(repo.root(), &Programs::processes(), (repo.run, &repo.head), at(21)).map(|_| ()),
    Err(ReviewRefusal::ImplementAtWork),
    "while the comments are on their way"
  );
  send::sent(repo.root(), &sending, Ok(()), at(22)).expect("sent");
  repo.return_to_implement(&ReviewNote::new("Rename one.").expect("note"), at(23));
  assert_eq!(
    approve_and_merge(repo.root(), &Programs::processes(), (repo.run, &repo.head), at(24)).map(|_| ()),
    Err(ReviewRefusal::ImplementAtWork)
  );
  assert!(repo.db().run_merge(repo.run).expect("read").is_none());
}
