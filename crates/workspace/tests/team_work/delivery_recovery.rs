//! Module delivery's recovery edge cases (team authority P4c hardening), against the fake server's delivery commands in
//! a real repository as [`crate::delivery`] makes it: a merge made before the computer stopped is found though the base
//! moved on since; the report and the request's operation are kept in one write; only another merge holding the
//! checkout makes the merge wait, any other fault is an error; a new request sends the reports waiting before it
//! first; and a merge that failed leaves the request's operation with its effect failed.

use std::fs;

use orchester_api::v2::DeliveryState;
use orchester_exec::local_git::IntegrationLease;
use orchester_store::{DeliveryMerge, ProjectDb, TeamDeliveryStage, TeamOperationState};
use orchester_workspace::team_work::{DeliveryProgress, DeliveryStatus, TeamWorkError};

use crate::{
  Checkout,
  delivery::{claim_module, commit, delivered, git, merged_tip, merges, person_deliveries, repository, request, state},
  team_server::{MODULE_BRANCH, TeamServer},
};

/// The stage the delivery under `operation` is recorded at, and the state of its request's team operation.
fn recorded(
  checkout: &Checkout,
  operation: orchester_core::id::OperationId,
) -> (TeamDeliveryStage, TeamOperationState) {
  let db = ProjectDb::open(checkout.root()).expect("open");
  let delivery = db.team_delivery(operation).expect("read").expect("recorded");
  let team = db.team_operation(operation).expect("read").expect("recorded");
  (delivery.stage, team.state)
}

/// Commits `file` on the branch checked out in the checkout, `main`.
fn commit_on_main(checkout: &Checkout, file: &str) -> String {
  fs::write(checkout.root().join(file), "later\n").expect("written");
  git(checkout.root(), &["add", file]);
  git(checkout.root(), &["commit", "-m", file]);
  git(checkout.root(), &["rev-parse", "main"])
}

/// Git merges the module branch as the delivery merges it, while the computer stops before it is recorded.
fn merge_by_hand(checkout: &Checkout) -> String {
  git(
    checkout.root(),
    &[
      "merge",
      "--no-ff",
      "--no-edit",
      "-m",
      "Merge module/payments",
      MODULE_BRANCH,
    ],
  );
  git(checkout.root(), &["rev-parse", "main"])
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn team_work_delivery_recovery_finds_the_merge_after_the_base_moved_on() {
  let server = TeamServer::start(crate::fixture::Config::new("Append one line."));
  let checkout = Checkout::linked(&server);
  let (base, head) = repository(&checkout);
  let claim = claim_module(&checkout, &server).await;
  server.state().automatic_merge = true;
  let (deliveries, generation) = person_deliveries(&checkout, &server);

  let lease = IntegrationLease::acquire(checkout.root()).expect("the checkout held");
  let crashed = request(&server, claim, (&base, &head), None);
  let progress = deliveries.request(crashed.clone()).await.expect("carried on");
  assert!(matches!(progress, DeliveryProgress::MergeWaiting), "{progress:?}");
  let merge = merge_by_hand(&checkout);
  merged_tip(checkout.root(), (&base, &head));
  // Before the delivery is carried on again, another commit lands on the base branch.
  let later = commit_on_main(&checkout, "later.txt");
  drop((lease, deliveries, generation));

  let (deliveries, _generation) = person_deliveries(&checkout, &server);
  let recovered = deliveries.recover().await.expect("recovered");
  let [(_, Ok(progress))] = <[_; 1]>::try_from(recovered).expect("one delivery") else {
    panic!("not carried on");
  };
  delivered(progress);
  assert_eq!(
    merges(checkout.root()),
    std::slice::from_ref(&merge),
    "not merged again"
  );
  assert_eq!(git(checkout.root(), &["rev-parse", "main"]), later);
  assert_eq!(
    server.state().deliveries[0].merge_commit.as_deref(),
    Some(merge.as_str())
  );
  assert_eq!(
    state(&deliveries, crashed.operation).merge,
    Some(DeliveryMerge::Merged { commit: commit(&merge) })
  );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn team_work_delivery_recovery_keeps_the_report_and_the_operation_in_one_write() {
  let server = TeamServer::start(crate::fixture::Config::new("Append one line."));
  let checkout = Checkout::linked(&server);
  let (base, head) = repository(&checkout);
  let claim = claim_module(&checkout, &server).await;
  server.state().automatic_merge = true;
  let (deliveries, _generation) = person_deliveries(&checkout, &server);

  let lease = IntegrationLease::acquire(checkout.root()).expect("the checkout held");
  let crashed = request(&server, claim, (&base, &head), None);
  let progress = deliveries.request(crashed.clone()).await.expect("carried on");
  assert!(matches!(progress, DeliveryProgress::MergeWaiting), "{progress:?}");
  drop(lease);
  // The computer stops as the request's operation is settled, after the report was kept.
  let raw = rusqlite::Connection::open(checkout.root().join(".orchester/orchester.db")).expect("open");
  raw
    .execute_batch(
      "CREATE TRIGGER stop_at_settling BEFORE UPDATE ON team_operations WHEN NEW.state = 'completed' \
       BEGIN SELECT RAISE(ABORT, 'the computer stopped'); END;",
    )
    .expect("the stop");
  let recovered = deliveries.recover().await.expect("recovered");
  assert!(
    matches!(recovered.as_slice(), [(_, Err(TeamWorkError::Store(_)))]),
    "{recovered:?}"
  );
  assert_eq!(
    recorded(&checkout, crashed.operation),
    (TeamDeliveryStage::Merging, TeamOperationState::Committed),
    "neither the report nor the settlement was kept"
  );
  let tip = merged_tip(checkout.root(), (&base, &head));
  assert_eq!(server.state().delivery_reports, 0);

  raw
    .execute_batch("DROP TRIGGER stop_at_settling;")
    .expect("started again");
  let recovered = deliveries.recover().await.expect("recovered");
  let [(_, Ok(progress))] = <[_; 1]>::try_from(recovered).expect("one delivery") else {
    panic!("not carried on");
  };
  delivered(progress);
  assert_eq!(merged_tip(checkout.root(), (&base, &head)), tip, "not merged again");
  assert_eq!(
    recorded(&checkout, crashed.operation),
    (TeamDeliveryStage::Reported, TeamOperationState::Completed)
  );
  let (dispatcher, _generation) = checkout.dispatcher(&server, crate::team_server::MEMBER);
  assert!(dispatcher.open_operations().expect("read").is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn team_work_delivery_recovery_a_hold_fault_is_an_error_not_a_wait() {
  let server = TeamServer::start(crate::fixture::Config::new("Append one line."));
  let checkout = Checkout::linked(&server);
  let (base, head) = repository(&checkout);
  let claim = claim_module(&checkout, &server).await;
  server.state().automatic_merge = true;
  let (deliveries, _generation) = person_deliveries(&checkout, &server);

  // The checkout's integration lock is a folder: no merge holds the checkout, and none ever can until it is fixed.
  let lock = checkout.root().join(".orchester/local-integration.lock");
  fs::create_dir(&lock).expect("the faulty lock");
  let faulty = request(&server, claim, (&base, &head), None);
  let outcome = deliveries.request(faulty.clone()).await;
  assert!(matches!(outcome, Err(TeamWorkError::DeliveryHold(_))), "{outcome:?}");
  assert_eq!(recorded(&checkout, faulty.operation).0, TeamDeliveryStage::Merging);
  assert!(merges(checkout.root()).is_empty(), "nothing merged");

  fs::remove_dir(&lock).expect("fixed");
  delivered(deliveries.request(faulty).await.expect("carried on"));
  merged_tip(checkout.root(), (&base, &head));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn team_work_delivery_recovery_a_request_sends_the_waiting_reports_first() {
  let server = TeamServer::start(crate::fixture::Config::new("Append one line."));
  let checkout = Checkout::linked(&server);
  let (base, head) = repository(&checkout);
  let claim = claim_module(&checkout, &server).await;
  server.state().automatic_merge = true;
  let (deliveries, _generation) = person_deliveries(&checkout, &server);

  let lease = IntegrationLease::acquire(checkout.root()).expect("the checkout held");
  let first = request(&server, claim, (&base, &head), None);
  let progress = deliveries.request(first.clone()).await.expect("carried on");
  assert!(matches!(progress, DeliveryProgress::MergeWaiting), "{progress:?}");
  drop(lease);
  server.set_down(true);
  let recovered = deliveries.recover().await.expect("recovered");
  assert!(
    matches!(
      recovered.as_slice(),
      [(_, Ok(DeliveryProgress::Waiting(TeamDeliveryStage::Reporting)))]
    ),
    "{recovered:?}"
  );
  let first_merge = commit(&merged_tip(checkout.root(), (&base, &head)));
  server.set_down(false);

  // The module branch goes on, and its next head is requested onto the first merge before anything else is sent.
  let root = checkout.root();
  git(root, &["switch", MODULE_BRANCH]);
  fs::write(root.join("refunds.txt"), "refunds\n").expect("refunds");
  git(root, &["add", "refunds.txt"]);
  git(root, &["commit", "-m", "Merge KON-8 run #1"]);
  let next_head = commit(&git(root, &["rev-parse", "HEAD"]));
  git(root, &["switch", "main"]);
  let second = request(&server, claim, (&first_merge, &next_head), None);
  delivered(deliveries.request(second.clone()).await.expect("carried on"));

  assert_eq!(state(&deliveries, first.operation).status, DeliveryStatus::Delivered);
  assert_eq!(state(&deliveries, second.operation).status, DeliveryStatus::Delivered);
  let server = server.state();
  assert_eq!(server.delivery_reports, 2);
  assert_eq!(server.deliveries[0].merge_commit.as_deref(), Some(first_merge.as_str()));
  assert_eq!(server.deliveries[1].state, "delivered");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn team_work_delivery_recovery_a_failed_merge_leaves_the_effect_failed() {
  let server = TeamServer::start(crate::fixture::Config::new("Append one line."));
  let checkout = Checkout::linked(&server);
  let (base, head) = repository(&checkout);
  let claim = claim_module(&checkout, &server).await;
  server.state().automatic_merge = true;
  let (deliveries, _generation) = person_deliveries(&checkout, &server);

  let lease = IntegrationLease::acquire(checkout.root()).expect("the checkout held");
  let moved = request(&server, claim, (&base, &head), None);
  let progress = deliveries.request(moved.clone()).await.expect("carried on");
  assert!(matches!(progress, DeliveryProgress::MergeWaiting), "{progress:?}");
  // The base branch moves past the authorized base commit: the merge refuses.
  commit_on_main(&checkout, "moved.txt");
  drop(lease);

  let recovered = deliveries.recover().await.expect("recovered");
  let [(_, Ok(DeliveryProgress::Reported(delivery)))] = <[_; 1]>::try_from(recovered).expect("one delivery") else {
    panic!("not reported");
  };
  assert_eq!(delivery.state, DeliveryState::Failed);
  assert!(merges(checkout.root()).is_empty(), "nothing merged");
  assert_eq!(
    recorded(&checkout, moved.operation),
    (TeamDeliveryStage::Reported, TeamOperationState::EffectFailed)
  );
  assert_eq!(state(&deliveries, moved.operation).status, DeliveryStatus::Failed);
}
