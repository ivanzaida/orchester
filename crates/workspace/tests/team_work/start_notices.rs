//! What the task's page is told of a start that no window asked about (ORC-885c): the runner's refusal that recovery
//! found is kept and read back until the runner answers otherwise; a refusal kept in the bare format reads as before;
//! a start settled for good is told once, until it is marked seen, and not at all when its reason no longer holds.

use orchester_api::v2::{AuthorityErrorCode, EXECUTION_BUNDLE_SCHEMA};
use orchester_core::id::{OperationId, TaskId, WorkId};
use orchester_ipc::Rejection;
use orchester_store::TeamStartStage;
use orchester_workspace::team_work::{
  ClaimEnd, LastStart, OpenStarts, StartFailure, StartOutcome, TeamWorkError, forget_runner_refusals, last_start,
  start_seen,
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
    last_start(checkout.root(), server.task)
      .expect("read")
      .map(|newest| newest.last),
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
    last_start(checkout.root(), server.task)
      .expect("read")
      .map(|newest| newest.last),
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
    last_start(checkout.root(), server.task)
      .expect("read")
      .map(|newest| newest.last),
    Some(LastStart::RunnerRefused(Rejection::Busy))
  ));

  // The runner cannot be asked this time: it did not refuse, so the page no longer says it did.
  let unavailable = work.carry_on_open(checkout.vault()).await.expect("unavailable");
  assert_eq!(unavailable, OpenStarts { changed: true });
  assert!(matches!(
    last_start(checkout.root(), server.task)
      .expect("read")
      .map(|newest| newest.last),
    Some(LastStart::Open)
  ));

  // The runner starts the run: the start is settled.
  let carried = work.carry_on_open(checkout.vault()).await.expect("started");
  assert_eq!(carried, OpenStarts { changed: true });
  assert!(matches!(
    last_start(checkout.root(), server.task)
      .expect("read")
      .map(|newest| newest.last),
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
  let Some(LastStart::Settled(StartOutcome::Refused(read, ended))) = last_start(checkout.root(), server.task)
    .expect("read")
    .map(|newest| newest.last)
  else {
    panic!("the refusal is read back");
  };
  assert_eq!((read, ended), (error, None));
}

/// The operation of the task's newest start.
fn operation_of(checkout: &Checkout, task: TaskId) -> OperationId {
  last_start(checkout.root(), task)
    .expect("read")
    .expect("a start")
    .operation
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
  let operation = operation_of(&checkout, server.task);
  start_seen(checkout.root(), server.task, operation).expect("an open start is left as it is");
  assert!(matches!(
    last_start(checkout.root(), server.task)
      .expect("read")
      .map(|newest| newest.last),
    Some(LastStart::Open)
  ));
  settle_failed(&checkout, &StartFailure::BundleUnreadable);
  assert!(matches!(
    last_start(checkout.root(), server.task)
      .expect("read")
      .map(|newest| newest.last),
    Some(LastStart::Settled(StartOutcome::Failed(
      StartFailure::BundleUnreadable,
      ClaimEnd::Released
    )))
  ));

  start_seen(checkout.root(), server.task, operation).expect("seen");
  start_seen(checkout.root(), server.task, operation).expect("seen again changes nothing");
  assert!(matches!(
    last_start(checkout.root(), server.task)
      .expect("read")
      .map(|newest| newest.last),
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
    last_start(checkout.root(), server.task)
      .expect("read")
      .map(|newest| newest.last),
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
    last_start(checkout.root(), server.task)
      .expect("read")
      .map(|newest| newest.last),
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn team_work_notices_a_dismissal_names_the_start_it_shows_and_a_newer_start_is_not_marked() {
  let server = TeamServer::start(Config::new("Append one line."));
  let checkout = Checkout::linked(&server);
  let claim = checkout.claim(&server, MEMBER).await;
  let (work, _generation) = checkout.work(&server, MEMBER, StoreRunner::new(checkout.root(), &[]));
  server.set_down(true);
  checkout
    .start(&work, checkout.start_request(&server, claim))
    .await
    .expect("waits");
  settle_failed(&checkout, &StartFailure::BundleUnreadable);
  let shown = operation_of(&checkout, server.task);

  // A later Run recorded another start while the page still shows the failure of the first.
  tokio::time::sleep(std::time::Duration::from_millis(5)).await;
  checkout
    .start(&work, checkout.start_request(&server, claim))
    .await
    .expect("waits");
  let newest = operation_of(&checkout, server.task);
  assert_ne!(newest, shown);

  let error = start_seen(checkout.root(), server.task, shown).expect_err("the start is not the newest any more");
  assert!(
    matches!(error, TeamWorkError::StartSuperseded(found) if found == shown),
    "{error:?}"
  );
  assert!(matches!(
    last_start(checkout.root(), server.task)
      .expect("read")
      .map(|newest| newest.last),
    Some(LastStart::Open)
  ));
  let kept = checkout
    .db()
    .team_start(newest)
    .expect("read")
    .expect("the newer start");
  assert!(
    !kept.seen,
    "the dismissal of the older start did not mark the newer one"
  );
  let older = checkout.db().team_start(shown).expect("read").expect("the older start");
  assert!(
    !older.seen,
    "and the older one was not marked either: the dismissal was refused"
  );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn team_work_notices_a_refusal_is_told_once_and_not_again_after_it_was_seen() {
  let server = TeamServer::start(Config::new("Append one line."));
  let checkout = Checkout::linked(&server);
  let claim = checkout.claim(&server, MEMBER).await;
  let (work, _generation) = checkout.work(&server, MEMBER, StoreRunner::new(checkout.root(), &[]));
  let request = checkout.start_request(&server, claim);
  server.drop_next_file();
  checkout.start(&work, request.clone()).await.expect("accepted, waits");
  take_over(&server, claim).await;
  checkout.start(&work, request).await.expect("refused");
  assert!(matches!(
    last_start(checkout.root(), server.task)
      .expect("read")
      .map(|newest| newest.last),
    Some(LastStart::Settled(StartOutcome::Refused(..)))
  ));

  let operation = operation_of(&checkout, server.task);
  start_seen(checkout.root(), server.task, operation).expect("seen");
  assert!(matches!(
    last_start(checkout.root(), server.task)
      .expect("read")
      .map(|newest| newest.last),
    Some(LastStart::Told(StartOutcome::Refused(..)))
  ));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn team_work_notices_a_refusal_kept_while_the_server_cannot_be_reached_is_forgotten() {
  let server = TeamServer::start(Config::new("Append one line."));
  let checkout = Checkout::linked(&server);
  let claim = checkout.claim(&server, MEMBER).await;
  let (work, _generation) = checkout.work(&server, MEMBER, StoreRunner::new(checkout.root(), &[Hiccup::Refuses]));
  server.set_down(true);
  checkout
    .start(&work, checkout.start_request(&server, claim))
    .await
    .expect("waits");
  assert_eq!(
    forget_runner_refusals(checkout.root()).expect("nothing kept"),
    OpenStarts { changed: false }
  );
  server.set_down(false);
  work.carry_on_open(checkout.vault()).await.expect("carried on");
  assert!(matches!(
    last_start(checkout.root(), server.task)
      .expect("read")
      .map(|newest| newest.last),
    Some(LastStart::RunnerRefused(Rejection::Busy))
  ));

  // The round that cannot reach the team server does not ask the runner: the page no longer says it refused.
  assert_eq!(
    forget_runner_refusals(checkout.root()).expect("forgotten"),
    OpenStarts { changed: true }
  );
  assert!(matches!(
    last_start(checkout.root(), server.task)
      .expect("read")
      .map(|newest| newest.last),
    Some(LastStart::Open)
  ));
  assert_eq!(checkout.db().open_team_starts().expect("open").len(), 1, "still open");
  assert_eq!(
    forget_runner_refusals(checkout.root()).expect("again"),
    OpenStarts { changed: false }
  );
}
