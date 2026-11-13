//! A team task in no module (the person's decision of 2026-11-13): its work starts on the task's revision alone, the
//! server builds its bundle without a module (schema 4), and its run starts on the branch this computer had checked
//! out when it asked, which it merges into as a local run does.

use orchester_api::v2::MODULE_LESS_BUNDLE_SCHEMA;
use orchester_core::{
  integration::base::RunBaseSource,
  run::{BranchName, inputs::InputsOrigin},
  store::RunStore,
};
use orchester_workspace::team_work::{StartFailure, StartOutcome};

use crate::{
  Checkout, StoreRunner,
  fixture::Config,
  team_server::{MEMBER, TeamServer},
};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn team_work_no_module_start_runs_on_the_branch_it_was_asked_on() {
  let server = TeamServer::start(Config::new("Append one line."));
  server.task_in_no_module();
  let checkout = Checkout::linked(&server);
  let claim = checkout.claim(&server, MEMBER).await;
  let (work, _generation) = checkout.work(&server, MEMBER, StoreRunner::new(checkout.root(), &[]));
  let mut request = checkout.start_request(&server, claim);
  request.work.module = None;
  request.base_branch = Some(BranchName::new("main").expect("branch"));
  let operation = request.operation;

  let outcome = checkout.start(&work, request).await.expect("started");
  let StartOutcome::Started(run) = outcome else {
    panic!("not started: {outcome:?}");
  };
  let db = checkout.db();
  let run = db.run(run).expect("read").expect("the run");
  assert_eq!(run.base_source(), RunBaseSource::CheckedOut);
  assert_eq!(run.base_branch().map(BranchName::as_str), Some("main"));
  // What the server accepted names no module, and the branch is this computer's.
  let accepted = db.accepted_team_inputs(operation).expect("read").expect("the inputs");
  let InputsOrigin::Team(acceptance) = accepted.inputs.bundle().origin() else {
    panic!("team inputs");
  };
  assert_eq!((acceptance.module, acceptance.branch.as_str()), (None, "main"));
  let schemas: Vec<u64> = server
    .state()
    .bundles
    .values()
    .filter_map(|(bundle, _)| bundle["schema_version"].as_u64())
    .collect();
  assert_eq!(schemas, [u64::from(MODULE_LESS_BUNDLE_SCHEMA)]);
}

/// A start recorded for a task in a module, which the server then answers with a bundle of no module (the task left
/// its module meanwhile), kept no branch for its run: it fails for good, and no run is made.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn team_work_no_module_bundle_for_a_start_that_kept_no_branch_fails_the_start() {
  let server = TeamServer::start(Config::new("Append one line."));
  server.task_in_no_module();
  let checkout = Checkout::linked(&server);
  let claim = checkout.claim(&server, MEMBER).await;
  let (work, _generation) = checkout.work(&server, MEMBER, StoreRunner::new(checkout.root(), &[]));
  let mut request = checkout.start_request(&server, claim);
  request.work.module = None;

  let outcome = checkout.start(&work, request).await.expect("carried on");
  assert!(
    matches!(&outcome, StartOutcome::Failed(StartFailure::Inputs(_), _)),
    "{outcome:?}"
  );
  let task = checkout.db().team_task_ids().expect("tasks");
  let runs: usize = task
    .iter()
    .map(|task| checkout.db().runs_of_task(*task).expect("runs").len())
    .sum();
  assert_eq!(runs, 0, "no run was made");
}
