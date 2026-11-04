//! A start carried on after a crash or an outage at each of its boundaries: the receipt kept, the bundle read, the
//! inputs kept, the runner's run. Each recovery goes on from the boundary it stopped at, once: one work, one run.

use orchester_api::v2::AuthorityErrorCode;
use orchester_core::{
  run::inputs::InputsOrigin,
  store::{BlobStore, RunStore},
};
use orchester_store::TeamStartStage;
use orchester_workspace::team_work::StartOutcome;

use crate::{
  Checkout, Hiccup, StoreRunner,
  fixture::{Config, NOTES},
  team_server::{MEMBER, TeamServer},
};

fn stage(checkout: &Checkout, outcome: &StartOutcome) -> TeamStartStage {
  let StartOutcome::Waiting(stage) = outcome else {
    panic!("not waiting: {outcome:?}");
  };
  assert!(checkout.db().open_team_starts().expect("open").len() == 1);
  *stage
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn team_work_start_recovers_once_from_every_boundary() {
  let config = Config::new("Append one line.");
  let server = TeamServer::start(config.clone());
  let checkout = Checkout::linked(&server);
  let claim = checkout.claim(&server, MEMBER).await;
  let request = checkout.start_request(&server, MEMBER, claim, &config).await;
  let operation = request.operation;
  let runner = StoreRunner::new(checkout.root(), &[Hiccup::Unavailable, Hiccup::StartsThenLost]);
  let (work, _generation) = checkout.work(&server, MEMBER, runner.clone());

  // The answer to `work.start` is lost: the operation stays open, the work made once.
  server.lose_next_response();
  let lost = work.start(request.clone()).await.expect("started");
  assert_eq!(stage(&checkout, &lost), TeamStartStage::Requested);
  assert_eq!(server.state().works_made, 1);

  // Recovered: looked up, the receipt's work and bundle kept; the bundle's file read is cut off.
  server.drop_next_file();
  let (_, cut) = work.recover().await.expect("recovered").remove(0);
  assert_eq!(stage(&checkout, &cut), TeamStartStage::Accepted);
  let kept = checkout.db().team_start(operation).expect("read").expect("kept");
  let (work_id, bundle) = kept.work.clone().expect("the receipt's work");
  assert_eq!(server.state().works[0].id, work_id);
  assert!(checkout.db().accepted_team_inputs(operation).expect("read").is_none());

  // Recovered: the bundle and its file read and kept with the inputs; the runner is not there.
  let (_, unavailable) = work.recover().await.expect("recovered").remove(0);
  assert!(
    matches!(unavailable, StartOutcome::RunnerUnavailable(_)),
    "{unavailable:?}"
  );
  let accepted = checkout
    .db()
    .accepted_team_inputs(operation)
    .expect("read")
    .expect("the inputs");
  let InputsOrigin::Team(acceptance) = accepted.inputs.bundle().origin() else {
    panic!("team inputs");
  };
  assert_eq!(
    (acceptance.work, &acceptance.bundle, acceptance.claim),
    (work_id, &bundle, claim)
  );
  assert_eq!(
    accepted.inputs.bundle().task_number().get(),
    crate::fixture::TASK_NUMBER
  );
  let file = accepted.inputs.bundle().inputs().files()[0].sha256.clone();
  assert_eq!(checkout.db().blob(&file).expect("the bundle's file"), NOTES);

  // Recovered: the runner starts the run and its answer is lost; then it answers with the same run.
  let (_, lost) = work.recover().await.expect("recovered").remove(0);
  assert!(matches!(lost, StartOutcome::RunnerUnavailable(_)), "{lost:?}");
  let (_, started) = work.recover().await.expect("recovered").remove(0);
  let StartOutcome::Started(run) = started else {
    panic!("not started: {started:?}");
  };
  assert_eq!(runner.asked(), 3);
  assert!(work.recover().await.expect("recovered").is_empty(), "nothing left open");
  let StartOutcome::Started(again) = work.start(request).await.expect("started") else {
    panic!("the same request carries on the recorded start");
  };
  assert_eq!(again, run);

  let db = checkout.db();
  assert_eq!(db.runs_of_task(accepted.task).expect("runs").len(), 1, "one run");
  assert_eq!(db.run_of_operation(operation).expect("read"), Some(run));
  assert_eq!(server.state().works_made, 1, "one work");
  assert!(db.team_run(run).expect("read").is_some());
  assert!(
    db.open_team_operations().expect("open").is_empty(),
    "the start effect happened"
  );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn team_work_start_refused_or_fenced_before_the_run_starts_nothing() {
  let config = Config::new("Append one line.");
  let server = TeamServer::start(config.clone());
  let checkout = Checkout::linked(&server);
  let claim = checkout.claim(&server, MEMBER).await;
  let runner = StoreRunner::new(checkout.root(), &[]);
  let (work, _generation) = checkout.work(&server, MEMBER, runner.clone());
  let request = checkout.start_request(&server, MEMBER, claim, &config).await;

  // Offline: the start waits, and nothing new starts without the server.
  server.set_down(true);
  let offline = work.start(request.clone()).await.expect("started");
  assert_eq!(stage(&checkout, &offline), TeamStartStage::Requested);
  server.set_down(false);

  // The receipt is kept, then another member takes the claim over before the bundle is read.
  server.drop_next_file();
  let cut = work.start(request.clone()).await.expect("started");
  assert_eq!(stage(&checkout, &cut), TeamStartStage::Accepted);
  crate::take_over(&server, claim).await;
  let refused = work.start(request.clone()).await.expect("carried on");
  let StartOutcome::Refused(error) = refused else {
    panic!("not refused: {refused:?}");
  };
  assert_eq!(error.code(), AuthorityErrorCode::ClaimLost);
  let StartOutcome::Refused(kept) = work.start(request).await.expect("carried on") else {
    panic!("the refusal is kept");
  };
  assert_eq!(kept, error);
  assert_eq!(runner.asked(), 0, "the runner was never asked");
  assert!(checkout.db().open_team_starts().expect("open").is_empty());
}
