//! The queue's merge attempts (ORC-361): each is recorded as the merge queue's, once while the same thing holds a
//! delivery back however often the queue polls it, with Orchester retrying it meanwhile; a base that moved under the
//! merge is told at the merge. `cargo test -p orchester-workspace --test merge_queue merge_attempts`.

use orchester_core::{
  integration::{
    DeliveredAs, Delivering,
    attempt::{AttemptOutcome, AttemptStage, MergeAsker, MergeVia},
  },
  merge_queue::{Blocked, QueueState},
  revision::Revision,
};
use orchester_exec::local_git::IntegrationLease;
use orchester_workspace::tasks::{integration::merge_in_as, merge_attempts::read};

use super::harness::{Repo, git};

const BY_QUEUE: (MergeAsker, MergeVia) = (MergeAsker::MergeQueue, MergeVia::MergeQueue);

#[test]
fn merge_attempts_of_the_queue_are_recorded_once_per_cause_and_retried_until_it_lands() {
  let mut repo = Repo::new();
  let task = repo.task("Sign in");
  let run = repo.succeeded((&task, "auto"), "orchester/orc-1-run-1", ("shared.txt", "run\n"), 100);
  let held = IntegrationLease::acquire(repo.root()).expect("another operation's lease");
  repo.drive();
  repo.drive();
  assert_eq!(repo.state(&run), QueueState::Blocked(Blocked::HeldLocally));
  let waiting = read::attempts_of(repo.root(), run.id()).expect("read");
  assert_eq!(waiting.len(), 1, "polled again, it is the same attempt: {waiting:?}");
  assert_eq!((waiting[0].attempt.asked_by(), waiting[0].attempt.via()), BY_QUEUE);
  assert_eq!(
    waiting[0].attempt.outcome(),
    &AttemptOutcome::Refused {
      stage: AttemptStage::IntegrationLock
    }
  );
  let not_merged = read::not_merged(repo.root()).expect("read");
  assert_eq!(not_merged.len(), 1);
  assert_eq!(
    not_merged[0].retrying.map(|retrying| retrying.next),
    Some(None),
    "the queue tries again by itself"
  );
  drop(held);
  repo.drive();
  let QueueState::Merged(delivered) = repo.state(&run) else {
    panic!("{:?}", repo.state(&run));
  };
  let recorded = read::attempts_of(repo.root(), run.id()).expect("read");
  assert_eq!(recorded.len(), 2);
  assert_eq!(
    recorded[0].attempt.outcome(),
    &AttemptOutcome::Merged {
      revision: delivered.revision,
      kind: delivered.kind,
    }
  );
  assert_eq!((recorded[0].attempt.asked_by(), recorded[0].attempt.via()), BY_QUEUE);
  assert!(read::not_merged(repo.root()).expect("read").is_empty());
}

#[test]
fn merge_attempts_a_base_that_moved_under_the_merge_is_told_at_the_merge() {
  let mut repo = Repo::new();
  let task = repo.task("Sign in");
  let run = repo.succeeded((&task, "auto"), "orchester/orc-1-run-1", ("shared.txt", "run\n"), 100);
  let checked = Revision::new(&git(repo.root(), &["rev-parse", "main"])).expect("revision");
  let tip = Revision::new(&git(repo.root(), &["rev-parse", "orchester/orc-1-run-1"])).expect("revision");
  Repo::commit_on_main(repo.root(), ("other.txt", "landed meanwhile\n"));
  let delivering = Delivering {
    as_delivered: DeliveredAs::MergeCommit,
    base: &checked,
  };
  merge_in_as(
    &repo.remover(),
    &repo.programs(),
    (run.id(), Some(&tip), Some(delivering)),
    BY_QUEUE,
  )
  .expect_err("the base moved");
  let attempt = &read::attempts_of(repo.root(), run.id()).expect("read")[0].attempt;
  assert_eq!(
    attempt.outcome(),
    &AttemptOutcome::Moved {
      stage: Some(AttemptStage::Merge)
    }
  );
}
