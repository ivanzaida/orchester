//! The queue and a merge's preflight (ORC-644): only the integration lock another operation holds makes a run wait;
//! any other stage that refuses stops it, with the stage and the cause.

use std::fs;

use orchester_core::merge_queue::{Blocked, Left, QueueState};
use orchester_exec::local_git::IntegrationLease;

use super::harness::Repo;

#[test]
fn merge_queue_preflight_stage_waits_while_another_operation_holds_the_integration_lock() {
  let mut repo = Repo::new();
  let task = repo.task("Sign in");
  let run = repo.succeeded((&task, "auto"), "orchester/orc-1-run-1", ("shared.txt", "run\n"), 100);
  let held = IntegrationLease::acquire(repo.root()).expect("another operation's lease");
  repo.drive();
  assert_eq!(repo.state(&run), QueueState::Blocked(Blocked::HeldLocally));
  drop(held);
  repo.drive();
  assert!(
    matches!(repo.state(&run), QueueState::Merged(_)),
    "{:?}",
    repo.state(&run)
  );
}

#[test]
fn merge_queue_preflight_stage_stops_the_run_with_any_other_stage_and_its_cause() {
  let mut repo = Repo::new();
  let task = repo.task("Sign in");
  let run = repo.succeeded((&task, "auto"), "orchester/orc-1-run-1", ("shared.txt", "run\n"), 100);
  let store = repo.root().join(".orchester");
  fs::write(store.join("elsewhere"), "").expect("another file");
  fs::hard_link(store.join("elsewhere"), store.join("orchester.db-journal")).expect("a second name for it");
  repo.drive();
  let QueueState::Left(Left::Stopped { detail }) = repo.state(&run) else {
    panic!("{:?}", repo.state(&run));
  };
  assert!(detail.starts_with("project_database: "), "{detail}");
}
