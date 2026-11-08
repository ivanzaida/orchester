//! What a task's newest start came to, read back from the project's records (ORC-885 follow-up): the page of a task
//! whose Run waited tells how it ended once recovery settled it, with what became of the claim, and says so for a
//! record this Orchester cannot read.

use orchester_api::v2::EXECUTION_BUNDLE_SCHEMA;
use orchester_workspace::team_work::{ClaimEnd, LastStart, OpenStarts, StartFailure, StartOutcome, last_start};
use serde_json::json;

use crate::{
  Checkout, StoreRunner,
  fixture::Config,
  team_server::{MEMBER, TeamServer},
};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn team_work_last_start_reads_back_a_failure_recovery_settled_with_its_claim() {
  let server = TeamServer::start(Config::new("Append one line."));
  let checkout = Checkout::linked(&server);
  let claim = checkout.claim(&server, MEMBER).await;
  let (work, _generation) = checkout.work(&server, MEMBER, StoreRunner::new(checkout.root(), &[]));
  let request = checkout.start_request(&server, claim);
  assert!(
    last_start(checkout.root(), server.task).expect("read").is_none(),
    "no start yet"
  );

  // The bundle's file read is cut off, so the start is accepted and waits; the bundle then turns out to be unreadable.
  server.drop_next_file();
  let waiting = checkout.start(&work, request).await.expect("started");
  assert!(matches!(waiting, StartOutcome::Waiting(_)), "{waiting:?}");
  assert!(matches!(
    last_start(checkout.root(), server.task).expect("read"),
    Some(LastStart::Open)
  ));
  {
    let mut state = server.state();
    let (bundle, _claim) = state.bundles.values_mut().next().expect("the start's bundle");
    *bundle = json!({"schema_version": EXECUTION_BUNDLE_SCHEMA});
  }

  let carried = work.carry_on_open(checkout.vault()).await.expect("carried on");
  assert_eq!(carried, OpenStarts { changed: true });
  let Some(LastStart::Settled(StartOutcome::Failed(failure, claim))) =
    last_start(checkout.root(), server.task).expect("read")
  else {
    panic!("the failure is read back");
  };
  assert_eq!((failure, claim), (StartFailure::BundleUnreadable, ClaimEnd::Released));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn team_work_last_start_keeps_what_became_of_the_claim_of_a_bundle_missing_refusal() {
  let server = TeamServer::start(Config::new("Append one line."));
  let checkout = Checkout::linked(&server);
  let claim = checkout.claim(&server, MEMBER).await;
  let (work, _generation) = checkout.work(&server, MEMBER, StoreRunner::new(checkout.root(), &[]));
  server.state().bundle_missing = Some(json!({}));

  let outcome = checkout
    .start(&work, checkout.start_request(&server, claim))
    .await
    .expect("started");
  let StartOutcome::Refused(refusal, ended) = outcome else {
    panic!("not refused: {outcome:?}");
  };
  assert_eq!(
    ended,
    Some(ClaimEnd::Released),
    "the refusal says the claim was let go of"
  );
  let Some(LastStart::Settled(StartOutcome::Refused(again, ended))) =
    last_start(checkout.root(), server.task).expect("read")
  else {
    panic!("the refusal is read back");
  };
  assert_eq!((again, ended), (refusal, Some(ClaimEnd::Released)));

  // A newer start of the task is the one read back, wherever it stands. Its record is a moment younger than the first.
  tokio::time::sleep(std::time::Duration::from_millis(5)).await;
  server.state().bundle_missing = None;
  let claim = checkout.claim(&server, MEMBER).await;
  server.set_down(true);
  checkout
    .start(&work, checkout.start_request(&server, claim))
    .await
    .expect("waits");
  assert!(matches!(
    last_start(checkout.root(), server.task).expect("read"),
    Some(LastStart::Open)
  ));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn team_work_last_start_says_a_record_it_cannot_read_and_carries_on_nothing() {
  let server = TeamServer::start(Config::new("Append one line."));
  let checkout = Checkout::linked(&server);
  let claim = checkout.claim(&server, MEMBER).await;
  let (work, _generation) = checkout.work(&server, MEMBER, StoreRunner::new(checkout.root(), &[]));
  server.set_down(true);
  checkout
    .start(&work, checkout.start_request(&server, claim))
    .await
    .expect("waits");
  rusqlite::Connection::open(checkout.root().join(".orchester/orchester.db"))
    .expect("open")
    .execute_batch("DROP TRIGGER team_starts_keep_request; UPDATE team_starts SET request = 'not a start'")
    .expect("the record is damaged");
  server.set_down(false);

  assert!(matches!(
    last_start(checkout.root(), server.task).expect("read"),
    Some(LastStart::Unreadable)
  ));
  assert_eq!(
    work.carry_on_open(checkout.vault()).await.expect("tried"),
    OpenStarts { changed: false }
  );
  assert!(matches!(
    last_start(checkout.root(), server.task).expect("read"),
    Some(LastStart::Unreadable)
  ));
}
