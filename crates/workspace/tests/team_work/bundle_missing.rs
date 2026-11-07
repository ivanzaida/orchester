//! A start the team server refuses `bundle_missing` is refused for good with the reason it gives: the refusal is kept
//! with the start as the server sent it, nothing waits, and no run is asked for. A refusal without a reason is kept
//! as it always was.

use orchester_api::v2::{AuthorityErrorCode, BundleMissingKind, ErrorDetails};
use orchester_core::id::ResourceId;
use orchester_store::TeamStartStage;
use orchester_workspace::team_work::StartOutcome;
use serde_json::{Value, json};

use crate::{
  Checkout, StoreRunner,
  fixture::Config,
  team_server::{MEMBER, TeamServer},
};

/// A refused start under `details`: the outcome, and the checkout and runner it was made with.
async fn refused_with(details: Value) -> (StartOutcome, Checkout, StoreRunner) {
  let config = Config::new("Append one line.");
  let server = TeamServer::start(config.clone());
  let checkout = Checkout::linked(&server);
  let claim = checkout.claim(&server, MEMBER).await;
  let runner = StoreRunner::new(checkout.root(), &[]);
  let (work, _generation) = checkout.work(&server, MEMBER, runner.clone());
  let request = checkout.start_request(&server, claim);
  server.state().bundle_missing = Some(details);
  let outcome = work.start(request.clone()).await.expect("started");
  let kept = checkout
    .db()
    .team_start(request.operation)
    .expect("read")
    .expect("kept");
  assert_eq!(kept.stage, TeamStartStage::Refused);
  let StartOutcome::Refused(again) = work.start(request).await.expect("carried on") else {
    panic!("the refusal is kept");
  };
  let StartOutcome::Refused(first) = &outcome else {
    panic!("not refused: {outcome:?}");
  };
  assert_eq!(&again, first, "kept as the server sent it");
  assert!(checkout.db().open_team_starts().expect("open").is_empty());
  assert_eq!(runner.asked(), 0, "the runner was never asked");
  (outcome, checkout, runner)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn team_work_bundle_missing_keeps_the_reason_the_server_gives() {
  let profile = ResourceId::generate();
  for kind in [
    "profile_unreadable",
    "mcp_server_archived",
    "file_missing",
    "body_unreadable",
  ] {
    let details = json!({"reason": {"kind": kind, "resource_id": profile, "version": 2}});
    let (StartOutcome::Refused(error), ..) = refused_with(details).await else {
      panic!("{kind}: not refused");
    };
    assert_eq!(error.code(), AuthorityErrorCode::BundleMissing, "{kind}");
    let ErrorDetails::BundleMissing { reason } = error.details() else {
      panic!("{kind}: no reason: {:?}", error.details());
    };
    assert_eq!(reason.resource_id, Some(profile), "{kind}");
    assert_eq!(reason.version.map(|version| version.get()), Some(2), "{kind}");
  }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn team_work_bundle_missing_unknown_kind_reads_as_other() {
  let details = json!({"reason": {"kind": "quota_exceeded", "limit": 3}});
  let (StartOutcome::Refused(error), ..) = refused_with(details).await else {
    panic!("not refused");
  };
  let ErrorDetails::BundleMissing { reason } = error.details() else {
    panic!("no reason");
  };
  assert_eq!(reason.kind, BundleMissingKind::Other);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn team_work_bundle_missing_without_a_reason_is_refused_as_before() {
  let (StartOutcome::Refused(error), ..) = refused_with(json!({})).await else {
    panic!("not refused");
  };
  assert_eq!(error.code(), AuthorityErrorCode::BundleMissing);
  assert_eq!(error.details(), &ErrorDetails::None);
}
