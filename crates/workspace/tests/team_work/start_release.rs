//! A start that fails for good lets go of its claim, so teammates can take the task at once; one that only waits for
//! the server keeps it, and so does a failure whose release the server could not take yet (ORC-885). The contract has
//! no report of a work that made no run, so the work is not reported.

use std::{
  path::PathBuf,
  sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
  },
};

use orchester_api::v2::EXECUTION_BUNDLE_SCHEMA;
use orchester_ipc::SessionEnd::Clean;
use orchester_store::TeamStartStage;
use orchester_workspace::{
  activity::Activity,
  runner::Runners,
  sharing::SharingError,
  team_work::{OpenStarts, OpenStartsError, StartFailure, StartOutcome, recover_open_starts},
};
use serde_json::json;
use tokio::runtime::Handle;

use crate::{
  Checkout, StoreRunner,
  fixture::Config,
  team_server::{Extra, MEMBER, TeamServer},
};

/// A server whose `claim.release` is answered by closing the connection while the returned switch is set.
fn server_cutting_releases() -> (TeamServer, Arc<AtomicBool>) {
  let cut = Arc::new(AtomicBool::new(false));
  let cutting = cut.clone();
  let extra: Extra = Arc::new(move |_, request, _| {
    let release = request.method == "POST"
      && request.path.ends_with("/commands")
      && request.json()["command"]["kind"] == "claim.release";
    release && cutting.load(Ordering::SeqCst)
  });
  (
    TeamServer::start_with(Config::new("Append one line."), Some(extra)),
    cut,
  )
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn team_work_release_a_start_that_only_waits_keeps_its_claim() {
  let server = TeamServer::start(Config::new("Append one line."));
  let checkout = Checkout::linked(&server);
  let claim = checkout.claim(&server, MEMBER).await;
  let (work, _generation) = checkout.work(&server, MEMBER, StoreRunner::new(checkout.root(), &[]));
  let request = checkout.start_request(&server, claim);

  server.set_down(true);
  let outcome = work.start(request, checkout.vault()).await.expect("started");
  assert!(matches!(outcome, StartOutcome::Waiting(_)), "{outcome:?}");
  assert_eq!(server.state().claim.as_ref().map(|held| held.id), Some(claim));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn team_work_release_waits_for_the_server_before_it_settles_the_failure() {
  let (server, cut) = server_cutting_releases();
  let checkout = Checkout::linked(&server);
  let claim = checkout.claim(&server, MEMBER).await;
  let runner = StoreRunner::new(checkout.root(), &[]);
  let (work, _generation) = checkout.work(&server, MEMBER, runner.clone());
  let request = checkout.start_request(&server, claim);

  // The bundle's file read is cut off, so the start is accepted; then the bundle turns out to be unreadable here, a
  // lasting failure, but the release is not answered.
  server.drop_next_file();
  work.start(request.clone(), checkout.vault()).await.expect("started");
  {
    let mut state = server.state();
    let (bundle, _claim) = state.bundles.values_mut().next().expect("the start's bundle");
    *bundle = json!({"schema_version": EXECUTION_BUNDLE_SCHEMA});
  }
  cut.store(true, Ordering::SeqCst);
  let waiting = work.start(request.clone(), checkout.vault()).await.expect("started");
  assert!(
    matches!(waiting, StartOutcome::Waiting(TeamStartStage::Accepted)),
    "{waiting:?}"
  );
  assert_eq!(checkout.db().open_team_starts().expect("open").len(), 1, "not settled");
  assert_eq!(
    server.state().claim.as_ref().map(|held| held.id),
    Some(claim),
    "still held"
  );

  // The server answers again: recovery settles the failure and the claim is let go of.
  cut.store(false, Ordering::SeqCst);
  let mut recovered = work.recover(checkout.vault()).await.expect("recovered");
  assert_eq!(recovered.len(), 1);
  let StartOutcome::Failed(failure) = recovered.remove(0).1.expect("carried on") else {
    panic!("not failed");
  };
  assert_eq!(failure, StartFailure::BundleUnreadable);
  assert!(server.state().claim.is_none(), "released");
  assert!(checkout.db().open_team_starts().expect("open").is_empty());
  assert_eq!(runner.asked(), 0, "no run was asked for");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn team_work_release_opening_a_project_without_open_starts_asks_nothing() {
  let server = TeamServer::start(Config::new("Append one line."));
  let checkout = Checkout::linked(&server);
  let runners = Runners::new(
    PathBuf::from("missing-runner"),
    Handle::current(),
    Activity::new(),
    Clean,
  );
  let opened = recover_open_starts(
    (&checkout.servers, runners),
    (checkout.root(), "Kontur"),
    checkout.vault(),
  )
  .await
  .expect("nothing to carry on");
  assert_eq!(opened, OpenStarts::None);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn team_work_release_an_open_start_without_a_server_session_is_kept_open() {
  let server = TeamServer::start(Config::new("Append one line."));
  let checkout = Checkout::linked(&server);
  let claim = checkout.claim(&server, MEMBER).await;
  let (work, _generation) = checkout.work(&server, MEMBER, StoreRunner::new(checkout.root(), &[]));
  server.set_down(true);
  work
    .start(checkout.start_request(&server, claim), checkout.vault())
    .await
    .expect("started");

  // The workspace of these tests knows no server entry to open a session in.
  let runners = Runners::new(
    PathBuf::from("missing-runner"),
    Handle::current(),
    Activity::new(),
    Clean,
  );
  let opened = recover_open_starts(
    (&checkout.servers, runners),
    (checkout.root(), "Kontur"),
    checkout.vault(),
  )
  .await;
  assert!(
    matches!(opened, Err(OpenStartsError::Sharing(SharingError::ServerMissing))),
    "{opened:?}"
  );
  assert_eq!(checkout.db().open_team_starts().expect("open").len(), 1, "still open");
}
