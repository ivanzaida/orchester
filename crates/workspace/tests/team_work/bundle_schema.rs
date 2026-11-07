//! A start whose bundle this computer cannot read settles failed with a reason, instead of waiting for a bundle that
//! reads the same every time: the receipt names a schema this computer does not read, or the bundle the server
//! answered is of another schema or is not a bundle at all. A server that cannot be reached still only waits.

use orchester_api::v2::EXECUTION_BUNDLE_SCHEMA;
use orchester_store::TeamStartStage;
use orchester_workspace::team_work::{StartFailure, StartOutcome, TeamStartRequest, TeamWork};
use serde_json::{Value, json};
use tokio::sync::watch;

use crate::{
  Checkout, StoreRunner,
  fixture::Config,
  team_server::{MEMBER, TeamServer},
};

const OLDER: u32 = EXECUTION_BUNDLE_SCHEMA - 1;

/// A server and a checkout with a claim, and a start held at `Accepted`: the bundle's file read is cut off, so the
/// bundle itself was read and decoded.
struct Accepted {
  server: TeamServer,
  checkout: Checkout,
  runner: StoreRunner,
  work: TeamWork<StoreRunner>,
  request: TeamStartRequest,
  /// The session's generation: the start is refused as changed once it is dropped.
  _generation: watch::Sender<u64>,
}

async fn accepted() -> Accepted {
  let config = Config::new("Append one line.");
  let server = TeamServer::start(config.clone());
  let checkout = Checkout::linked(&server);
  let claim = checkout.claim(&server, MEMBER).await;
  let runner = StoreRunner::new(checkout.root(), &[]);
  let (work, generation) = checkout.work(&server, MEMBER, runner.clone());
  let request = checkout.start_request(&server, claim);
  server.drop_next_file();
  let cut = work.start(request.clone(), checkout.vault()).await.expect("started");
  assert!(
    matches!(cut, StartOutcome::Waiting(TeamStartStage::Accepted)),
    "{cut:?}"
  );
  Accepted {
    server,
    checkout,
    runner,
    work,
    request,
    _generation: generation,
  }
}

/// The start under `held` is failed with `expected`, kept so, and made no inputs and no run.
async fn assert_failed(held: &Accepted, outcome: StartOutcome, expected: &StartFailure) {
  let StartOutcome::Failed(failure) = outcome else {
    panic!("not failed: {outcome:?}");
  };
  assert_eq!(&failure, expected);
  let operation = held.request.operation;
  let db = held.checkout.db();
  let kept = db.team_start(operation).expect("read").expect("kept");
  assert_eq!(kept.stage, TeamStartStage::Failed);
  assert!(db.accepted_team_inputs(operation).expect("read").is_none());
  assert!(db.open_team_starts().expect("open").is_empty(), "nothing left open");
  let StartOutcome::Failed(again) = held
    .work
    .start(held.request.clone(), held.checkout.vault())
    .await
    .expect("carried on")
  else {
    panic!("the failure is kept");
  };
  assert_eq!(&again, expected);
  assert!(
    held
      .work
      .recover(held.checkout.vault())
      .await
      .expect("recovered")
      .is_empty()
  );
  assert_eq!(held.runner.asked(), 0, "the runner was never asked");
  assert!(
    held.server.state().claim.is_none(),
    "the claim was released with the failure"
  );
}

fn set_body(held: &Accepted, body: impl FnOnce(&mut Value)) {
  let mut state = held.server.state();
  let (bundle, _claim) = state.bundles.values_mut().next().expect("the start's bundle");
  body(bundle);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn team_work_bundle_schema_of_the_receipt_fails_the_start_before_the_bundle_is_read() {
  let config = Config::new("Append one line.");
  let server = TeamServer::start(config.clone());
  let checkout = Checkout::linked(&server);
  let claim = checkout.claim(&server, MEMBER).await;
  let runner = StoreRunner::new(checkout.root(), &[]);
  let (work, generation) = checkout.work(&server, MEMBER, runner.clone());
  let request = checkout.start_request(&server, claim);

  server.state().receipt_schema = Some(OLDER);
  let outcome = work.start(request.clone(), checkout.vault()).await.expect("started");
  let held = Accepted {
    server,
    checkout,
    runner,
    work,
    request,
    _generation: generation,
  };
  let expected = StartFailure::BundleSchema {
    read: EXECUTION_BUNDLE_SCHEMA,
    built: OLDER,
  };
  assert_failed(&held, outcome, &expected).await;
  assert_eq!(held.server.state().works_made, 1, "the server made its work once");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn team_work_bundle_schema_of_the_answered_bundle_fails_the_start() {
  let held = accepted().await;
  set_body(&held, |bundle| bundle["schema_version"] = json!(OLDER));
  let outcome = held
    .work
    .start(held.request.clone(), held.checkout.vault())
    .await
    .expect("carried on");
  let expected = StartFailure::BundleSchema {
    read: EXECUTION_BUNDLE_SCHEMA,
    built: OLDER,
  };
  assert_failed(&held, outcome, &expected).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn team_work_bundle_unreadable_json_fails_the_start_and_recovery_passes_it_by() {
  let held = accepted().await;
  set_body(&held, |bundle| {
    *bundle = json!({"schema_version": EXECUTION_BUNDLE_SCHEMA})
  });
  let recovered = held.work.recover(held.checkout.vault()).await.expect("recovered");
  assert_eq!(recovered.len(), 1, "one open start");
  let outcome = recovered.into_iter().next().expect("one").1.expect("carried on");
  assert_failed(&held, outcome, &StartFailure::BundleUnreadable).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn team_work_bundle_unreachable_server_still_waits_at_the_boundary() {
  let held = accepted().await;
  held.server.set_down(true);
  let down = held
    .work
    .start(held.request.clone(), held.checkout.vault())
    .await
    .expect("carried on");
  assert!(
    matches!(down, StartOutcome::Waiting(TeamStartStage::Accepted)),
    "{down:?}"
  );
  assert_eq!(held.checkout.db().open_team_starts().expect("open").len(), 1);
  held.server.set_down(false);
  let after = held
    .work
    .start(held.request.clone(), held.checkout.vault())
    .await
    .expect("carried on");
  assert!(
    matches!(after, StartOutcome::RunnerUnavailable(_) | StartOutcome::Started(_)),
    "{after:?}"
  );
}
