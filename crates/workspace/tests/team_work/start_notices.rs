//! What the task's page is told of a start that no window asked about (ORC-885c): the runner's refusal that recovery
//! found is kept and read back until the runner answers otherwise; a refusal kept in the bare format reads as before;
//! a start settled for good is told once, until it is marked seen, and not at all when its reason no longer holds.

use orchester_api::v2::{AuthorityErrorCode, EXECUTION_BUNDLE_SCHEMA};
use orchester_core::id::WorkId;
use orchester_ipc::Rejection;
use orchester_store::TeamStartStage;
use orchester_workspace::team_work::{
  ClaimEnd, LastStart, OpenStarts, StartFailure, StartOutcome, last_start, start_seen,
};
use serde_json::json;

use crate::{
  Checkout, Hiccup, StoreRunner,
  fixture::Config,
  take_over,
  team_server::{MEMBER, TeamServer},
};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn team_work_notices_a_runner_refusal_of_recovery_is_kept_until_the_runner_answers_otherwise() {
  let server = TeamServer::start(Config::new("Append one line."));
  let checkout = Checkout::linked(&server);
  let claim = checkout.claim(&server, MEMBER).await;
  let runner = StoreRunner::new(
    checkout.root(),
    &[Hiccup::Refuses, Hiccup::Refuses, Hiccup::Unavailable],
  );
  let (work, _generation) = checkout.work(&server, MEMBER, runner);
  server.set_down(true);
  checkout
    .start(&work, checkout.start_request(&server, claim))
    .await
    .expect("waits");
  assert!(matches!(
    last_start(checkout.root(), server.task).expect("read"),
    Some(LastStart::Open)
  ));
  server.set_down(false);

  // The server answers, and the runner refuses the kept inputs: nothing settled, and the page can say what it said.
  let carried = work.carry_on_open(checkout.vault()).await.expect("carried on");
  assert_eq!(
    carried,
    OpenStarts { changed: true },
    "the page is told the runner refused"
  );
  assert!(matches!(
    last_start(checkout.root(), server.task).expect("read"),
    Some(LastStart::RunnerRefused(Rejection::Busy))
  ));
  assert_eq!(checkout.db().open_team_starts().expect("open").len(), 1, "still open");

  // Asked again, it refuses again: the same answer, not a second record.
  let again = work.carry_on_open(checkout.vault()).await.expect("again");
  assert_eq!(
    again,
    OpenStarts { changed: false },
    "the same answer is not told again"
  );
  assert!(matches!(
    last_start(checkout.root(), server.task).expect("read"),
    Some(LastStart::RunnerRefused(Rejection::Busy))
  ));

  // The runner cannot be asked this time: it did not refuse, so the page no longer says it did.
  let unavailable = work.carry_on_open(checkout.vault()).await.expect("unavailable");
  assert_eq!(unavailable, OpenStarts { changed: true });
  assert!(matches!(
    last_start(checkout.root(), server.task).expect("read"),
    Some(LastStart::Open)
  ));

  // The runner starts the run: the start is settled.
  let carried = work.carry_on_open(checkout.vault()).await.expect("started");
  assert_eq!(carried, OpenStarts { changed: true });
  assert!(matches!(
    last_start(checkout.root(), server.task).expect("read"),
    Some(LastStart::Settled(StartOutcome::Started(_)))
  ));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn team_work_notices_a_refusal_kept_in_the_bare_format_reads_as_a_refusal_without_a_claim_end() {
  let server = TeamServer::start(Config::new("Append one line."));
  let checkout = Checkout::linked(&server);
  let claim = checkout.claim(&server, MEMBER).await;
  let (work, _generation) = checkout.work(&server, MEMBER, StoreRunner::new(checkout.root(), &[]));
  let request = checkout.start_request(&server, claim);
  server.drop_next_file();
  checkout.start(&work, request.clone()).await.expect("accepted, waits");
  take_over(&server, claim).await;
  let StartOutcome::Refused(error, ended) = checkout.start(&work, request).await.expect("refused") else {
    panic!("not refused");
  };
  assert_eq!((error.code(), ended), (AuthorityErrorCode::ClaimLost, None));

  // As ORC-885 wrote it: the server's refusal alone.
  let record = checkout
    .db()
    .latest_team_start(server.task)
    .expect("read")
    .expect("a start");
  assert_eq!(record.stage, TeamStartStage::Refused);
  assert_eq!(
    record.refusal.as_deref(),
    Some(serde_json::to_string(&error).expect("json").as_str())
  );
  let Some(LastStart::Settled(StartOutcome::Refused(read, ended))) =
    last_start(checkout.root(), server.task).expect("read")
  else {
    panic!("the refusal is read back");
  };
  assert_eq!((read, ended), (error, None));
}

/// Settles the task's newest start failed with `failure`, as recovery would have.
fn settle_failed(checkout: &Checkout, failure: &StartFailure) {
  let kept = json!({"failure": failure, "claim": ClaimEnd::Released}).to_string();
  rusqlite::Connection::open(checkout.root().join(".orchester/orchester.db"))
    .expect("open")
    .execute(
      "UPDATE team_starts SET stage = 'failed', work_id = ?1, bundle_sha256 = ?2, failure = ?3",
      rusqlite::params![WorkId::generate().to_string(), "0".repeat(64), kept],
    )
    .expect("the start is settled");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn team_work_notices_a_failure_is_told_once_and_not_again_after_it_was_seen() {
  let server = TeamServer::start(Config::new("Append one line."));
  let checkout = Checkout::linked(&server);
  let claim = checkout.claim(&server, MEMBER).await;
  let (work, _generation) = checkout.work(&server, MEMBER, StoreRunner::new(checkout.root(), &[]));
  server.set_down(true);
  checkout
    .start(&work, checkout.start_request(&server, claim))
    .await
    .expect("waits");
  start_seen(checkout.root(), server.task).expect("an open start is left as it is");
  assert!(matches!(
    last_start(checkout.root(), server.task).expect("read"),
    Some(LastStart::Open)
  ));
  settle_failed(&checkout, &StartFailure::BundleUnreadable);
  assert!(matches!(
    last_start(checkout.root(), server.task).expect("read"),
    Some(LastStart::Settled(StartOutcome::Failed(
      StartFailure::BundleUnreadable,
      ClaimEnd::Released
    )))
  ));

  start_seen(checkout.root(), server.task).expect("seen");
  start_seen(checkout.root(), server.task).expect("seen again changes nothing");
  assert!(matches!(
    last_start(checkout.root(), server.task).expect("read"),
    Some(LastStart::Told(StartOutcome::Failed(StartFailure::BundleUnreadable, _)))
  ));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn team_work_notices_a_schema_failure_is_not_told_once_this_app_reads_that_schema() {
  let server = TeamServer::start(Config::new("Append one line."));
  let checkout = Checkout::linked(&server);
  let claim = checkout.claim(&server, MEMBER).await;
  let (work, _generation) = checkout.work(&server, MEMBER, StoreRunner::new(checkout.root(), &[]));
  server.set_down(true);
  checkout
    .start(&work, checkout.start_request(&server, claim))
    .await
    .expect("waits");

  // The server built a schema a release ahead: still the reason.
  let ahead = StartFailure::BundleSchema {
    read: EXECUTION_BUNDLE_SCHEMA,
    built: EXECUTION_BUNDLE_SCHEMA + 1,
  };
  assert!(ahead.holds());
  settle_failed(&checkout, &ahead);
  assert!(matches!(
    last_start(checkout.root(), server.task).expect("read"),
    Some(LastStart::Settled(StartOutcome::Failed(..)))
  ));

  // Orchester was updated since: it reads the schema the failure names, which is no longer the reason.
  let updated = StartFailure::BundleSchema {
    read: EXECUTION_BUNDLE_SCHEMA - 1,
    built: EXECUTION_BUNDLE_SCHEMA,
  };
  assert!(!updated.holds());
  settle_failed_again(&checkout, &updated);
  assert!(matches!(
    last_start(checkout.root(), server.task).expect("read"),
    Some(LastStart::Told(StartOutcome::Failed(..)))
  ));
}

/// As [`settle_failed`], for a start that is settled already: the trigger that keeps a settled start is dropped.
fn settle_failed_again(checkout: &Checkout, failure: &StartFailure) {
  rusqlite::Connection::open(checkout.root().join(".orchester/orchester.db"))
    .expect("open")
    .execute_batch("DROP TRIGGER team_starts_keep_request")
    .expect("the record is opened");
  settle_failed(checkout, failure);
}
