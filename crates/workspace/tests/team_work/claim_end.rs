//! What becomes of a task's claim when a start fails for good (ORC-885): it is let go of unless a run that still
//! works or has an outcome to report is under it, and a release the server refuses is not called one.

use std::sync::Arc;

use orchester_api::v2::EXECUTION_BUNDLE_SCHEMA;
use orchester_core::{
  id::{ClaimId, RunId},
  run::{RunStatus, StepFailure},
  store::RunStore,
};
use orchester_store::TeamStartStage;
use orchester_workspace::team_work::{ClaimEnd, StartFailure, StartOutcome, TeamWork};
use serde_json::json;

use crate::{
  Checkout, StoreRunner,
  fixture::Config,
  http::respond,
  started,
  team_server::{Extra, MEMBER, TeamServer, refusal},
};

/// The run finished successfully, as the runner records it.
fn succeed(checkout: &Checkout, run: RunId) {
  for pass in [false, true] {
    checkout.change_run(run, |run, now| {
      let execution = run.current_attempt().expect("attempt").executions()[0].id();
      if pass {
        run.complete_step(execution, now)
      } else {
        run.start_step(execution, now)
      }
    });
  }
  assert_eq!(
    checkout.db().run(run).expect("read").expect("the run").status(),
    &RunStatus::Succeeded
  );
}

/// The run's step failed, as the runner records it: a person can recover the run.
fn fail(checkout: &Checkout, run: RunId) {
  for pass in [false, true] {
    checkout.change_run(run, |run, now| {
      let execution = run.current_attempt().expect("attempt").executions()[0].id();
      if pass {
        run.fail_step(execution, StepFailure::ReportedFailed, now)
      } else {
        run.start_step(execution, now)
      }
    });
  }
  let loaded = checkout.db().run(run).expect("read").expect("the run");
  assert!(matches!(loaded.status(), RunStatus::Failed(_)), "{:?}", loaded.status());
  assert!(loaded.recovery_step(None).is_some(), "a person can recover it");
}

/// A second start of the task under `claim`, whose bundle turns out unreadable here: the outcome it ends in.
async fn failing_second_start(
  (server, checkout): (&TeamServer, &Checkout),
  work: &TeamWork<StoreRunner>,
  claim: ClaimId,
) -> StartOutcome {
  let request = checkout.start_request(server, claim);
  server.drop_next_file();
  let waiting = checkout.start(work, request.clone()).await.expect("started");
  assert!(
    matches!(waiting, StartOutcome::Waiting(TeamStartStage::Accepted)),
    "{waiting:?}"
  );
  for (bundle, _claim) in server.state().bundles.values_mut() {
    *bundle = json!({"schema_version": EXECUTION_BUNDLE_SCHEMA});
  }
  checkout.start(work, request).await.expect("carried on")
}

/// The server no longer runs the work of the first start (it ended it on its side), so it takes another under the
/// claim.
fn server_ends_its_work(server: &TeamServer) {
  for work in &mut server.state().works {
    work.state = "failed".to_owned();
  }
}

fn claim_of(server: &TeamServer) -> Option<ClaimId> {
  server.state().claim.as_ref().map(|held| held.id)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn team_work_claim_end_a_failure_after_a_finished_and_reported_run_releases_the_claim() {
  let server = TeamServer::start(Config::new("Append one line."));
  let checkout = Checkout::linked(&server);
  let (work, run, _generation) = started(&checkout, &server).await;
  let claim = claim_of(&server).expect("held");
  succeed(&checkout, run);
  let reported = work.report().await.expect("reported");
  assert!(!reported.waiting, "{reported:?}");

  let outcome = failing_second_start((&server, &checkout), &work, claim).await;
  let StartOutcome::Failed(failure, end) = outcome else {
    panic!("not failed: {outcome:?}");
  };
  assert_eq!(failure, StartFailure::BundleUnreadable);
  assert_eq!(end, ClaimEnd::Released);
  assert!(server.state().claim.is_none(), "released");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn team_work_claim_end_a_failed_run_a_person_can_recover_keeps_the_claim_though_its_outcome_is_sent() {
  let server = TeamServer::start(Config::new("Append one line."));
  let checkout = Checkout::linked(&server);
  let (work, run, _generation) = started(&checkout, &server).await;
  let claim = claim_of(&server).expect("held");
  fail(&checkout, run);
  let reported = work.report().await.expect("reported");
  assert!(!reported.waiting, "{reported:?}");

  let outcome = failing_second_start((&server, &checkout), &work, claim).await;
  let StartOutcome::Failed(_, end) = outcome else {
    panic!("not failed: {outcome:?}");
  };
  assert_eq!(end, ClaimEnd::Kept, "recovering the run needs its claim");
  assert_eq!(claim_of(&server), Some(claim), "still held");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn team_work_claim_end_a_finished_run_whose_outcome_is_not_sent_yet_keeps_the_claim() {
  let server = TeamServer::start(Config::new("Append one line."));
  let checkout = Checkout::linked(&server);
  let (work, run, _generation) = started(&checkout, &server).await;
  let claim = claim_of(&server).expect("held");
  succeed(&checkout, run);
  server_ends_its_work(&server);

  let outcome = failing_second_start((&server, &checkout), &work, claim).await;
  let StartOutcome::Failed(_, end) = outcome else {
    panic!("not failed: {outcome:?}");
  };
  assert_eq!(end, ClaimEnd::Kept);
  assert_eq!(claim_of(&server), Some(claim), "still held");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn team_work_claim_end_a_failure_beside_a_running_run_keeps_the_claim_and_says_so() {
  let server = TeamServer::start(Config::new("Append one line."));
  let checkout = Checkout::linked(&server);
  let (work, _run, _generation) = started(&checkout, &server).await;
  let claim = claim_of(&server).expect("held");
  server_ends_its_work(&server);

  let outcome = failing_second_start((&server, &checkout), &work, claim).await;
  let StartOutcome::Failed(failure, end) = outcome else {
    panic!("not failed: {outcome:?}");
  };
  assert_eq!(failure, StartFailure::BundleUnreadable);
  assert_eq!(end, ClaimEnd::Kept);
  assert_eq!(claim_of(&server), Some(claim), "still held");
}

/// A server that refuses every `claim.release` with `status` and `code` while it runs: `invalid` is what a server
/// older than the rule answers for running work (the claim's `AgentWorking`), and what any server answers for work with
/// an accepted report.
fn server_refusing_releases(status: u16, code: &'static str) -> TeamServer {
  let extra: Extra = Arc::new(move |_, request, stream| {
    let release = request.method == "POST"
      && request.path.ends_with("/commands")
      && request.json()["command"]["kind"] == "claim.release";
    if release {
      let operation = request.json()["operation_id"].as_str().map(str::to_owned);
      respond(stream, status, &refusal(operation.as_deref(), code));
    }
    release
  });
  TeamServer::start_with(Config::new("Append one line."), Some(extra))
}

/// A start that fails after `work.start` while the server refuses the release as `status` and `code` say: its end.
async fn held_after_refusal(status: u16, code: &'static str) -> (TeamServer, ClaimId) {
  let server = server_refusing_releases(status, code);
  let checkout = Checkout::linked(&server);
  let claim = checkout.claim(&server, MEMBER).await;
  let (work, _generation) = checkout.work(&server, MEMBER, StoreRunner::new(checkout.root(), &[]));

  let outcome = failing_second_start((&server, &checkout), &work, claim).await;
  let StartOutcome::Failed(_, end) = outcome else {
    panic!("not failed: {outcome:?}");
  };
  assert_eq!(end, ClaimEnd::Held);
  (server, claim)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn team_work_claim_end_a_failure_after_work_start_with_no_report_ends_the_work_and_releases_the_claim() {
  let server = TeamServer::start(Config::new("Append one line."));
  let checkout = Checkout::linked(&server);
  let claim = checkout.claim(&server, MEMBER).await;
  let (work, _generation) = checkout.work(&server, MEMBER, StoreRunner::new(checkout.root(), &[]));

  let outcome = failing_second_start((&server, &checkout), &work, claim).await;
  let StartOutcome::Failed(failure, end) = outcome else {
    panic!("not failed: {outcome:?}");
  };
  assert_eq!(failure, StartFailure::BundleUnreadable);
  assert_eq!(end, ClaimEnd::Released, "the work is running, but it has no report");
  assert!(server.state().claim.is_none(), "released");
  let state = server.state();
  assert_eq!(
    state.works.last().map(|work| work.state.as_str()),
    Some("failed"),
    "the release ended the work the start made"
  );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn team_work_claim_end_a_release_the_server_refuses_for_good_is_not_a_release() {
  let (server, claim) = held_after_refusal(403, "forbidden").await;
  assert_eq!(claim_of(&server), Some(claim), "the server still holds it");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn team_work_claim_end_a_server_that_refuses_to_release_running_work_leaves_the_claim_held() {
  let (server, claim) = held_after_refusal(422, "invalid").await;
  assert_eq!(claim_of(&server), Some(claim), "an older server keeps the claim");
  assert_eq!(
    server.state().works.last().map(|work| work.state.as_str()),
    Some("running"),
    "and the work"
  );
}
