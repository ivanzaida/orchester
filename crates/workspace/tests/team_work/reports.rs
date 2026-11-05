//! Work reports: queued in the project database before they are sent, sent in order, waiting across an outage and a
//! restart, never sent twice; the outcomes as decided; a takeover fences the run and keeps its output here; and the
//! run's local status apart from what the server accepted.

use orchester_api::v2::ExecutionState;
use orchester_core::{
  run::{PauseReason, RunLimit, RunStatus},
  store::RunStore,
};
use orchester_store::{TeamFence, WorkReportState};
use orchester_workspace::team_work::Reported;

use crate::{
  Checkout, StoreRunner, started,
  team_server::{MEMBER, TeamServer},
};

fn accepted(server: &TeamServer) -> Vec<(u64, String)> {
  server.state().accepted.clone()
}

fn kinds(list: &[(u64, &str)]) -> Vec<(u64, String)> {
  list
    .iter()
    .map(|(sequence, kind)| (*sequence, (*kind).to_owned()))
    .collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn team_work_reports_wait_in_order_across_an_outage_and_a_restart() {
  let server = TeamServer::start(crate::fixture::Config::new("Append one line."));
  let checkout = Checkout::linked(&server);
  let (work, run, _generation) = started(&checkout, &server).await;
  let reported = work.report().await.expect("reported");
  assert_eq!((reported.queued, reported.acknowledged), (1, 1));
  assert_eq!(accepted(&server), kinds(&[(1, "progress")]));
  assert_eq!(
    work.report().await.expect("reported"),
    Reported::default(),
    "nothing changed"
  );

  // Offline, the run goes on here; its reports wait.
  server.set_down(true);
  checkout.change_run(run, |run, now| run.stop_at_limit(RunLimit::DailyBudget, now));
  let waiting = work.report().await.expect("reported");
  assert_eq!((waiting.queued, waiting.acknowledged, waiting.waiting), (1, 0, true));
  let state = work.run_state(run).expect("read").expect("a team run");
  assert_eq!(
    state.local,
    RunStatus::Paused(PauseReason::LimitReached(RunLimit::DailyBudget))
  );
  assert_eq!(
    state.accepted.map(|work| work.state),
    Some(ExecutionState::Running),
    "the server's acceptance, apart"
  );
  assert_eq!(state.waiting_reports, 1);
  drop(work);

  // Restarted, still offline: the run resumes and is cancelled; each change waits behind the first.
  let (work, _generation) = checkout.work(&server, MEMBER, StoreRunner::new(checkout.root(), &[]));
  checkout.change_run(run, |run, now| run.resume(now));
  checkout.change_run(run, |run, now| run.cancel(now));
  let waiting = work.report().await.expect("reported");
  assert_eq!(
    (waiting.queued, waiting.waiting),
    (1, true),
    "the run is cancelled before the pass reads it"
  );
  assert_eq!(accepted(&server).len(), 1);

  // Back online; the first answer is lost, so the report is sent again with the same event and sequence.
  server.set_down(false);
  server.lose_next_response();
  let lost = work.report().await.expect("reported");
  assert_eq!((lost.acknowledged, lost.waiting), (0, true));
  let sent = work.report().await.expect("reported");
  assert_eq!((sent.acknowledged, sent.refused, sent.waiting), (2, 0, false));
  assert_eq!(
    accepted(&server),
    kinds(&[(1, "progress"), (2, "failed"), (3, "cancelled")]),
    "in order, each once"
  );
  let state = work.run_state(run).expect("read").expect("a team run");
  assert_eq!(state.accepted.map(|work| work.state), Some(ExecutionState::Cancelled));
  assert_eq!((state.waiting_reports, state.refused_reports), (0, 0));
  assert!(
    server.state().claim.is_none(),
    "a cancellation releases the independent claim"
  );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn team_work_a_reached_limit_stays_resumable_and_failures_keep_the_claim() {
  let server = TeamServer::start(crate::fixture::Config::new("Append one line."));
  let checkout = Checkout::linked(&server);
  let (work, run, _generation) = started(&checkout, &server).await;
  checkout.change_run(run, |run, now| run.stop_at_limit(RunLimit::DailyBudget, now));
  work.report().await.expect("reported");
  assert_eq!(server.state().works[0].state, "paused", "a reached limit is a pause");
  checkout.change_run(run, |run, now| run.resume(now));
  work.report().await.expect("reported");
  assert_eq!(server.state().works[0].state, "running", "resumed");
  checkout.change_run(run, |run, now| {
    run.interrupt(orchester_core::run::InterruptCause::RunnerFailed, now)
  });
  work.report().await.expect("reported");
  let state = server.state();
  assert_eq!(state.works[0].state, "failed");
  assert!(state.claim.is_some(), "a failure keeps the claim");
  assert_eq!(state.accepted, kinds(&[(1, "failed"), (2, "progress"), (3, "failed")]));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn team_work_a_takeover_fences_the_run_and_keeps_its_output() {
  let server = TeamServer::start(crate::fixture::Config::new("Append one line."));
  let checkout = Checkout::linked(&server);
  let (work, run, _generation) = started(&checkout, &server).await;
  work.report().await.expect("reported");
  let claim = server.state().claim.clone().expect("held").id;
  crate::take_over(&server, claim).await;

  checkout.change_run(run, |run, now| run.stop_at_limit(RunLimit::DailyBudget, now));
  let fenced = work.report().await.expect("reported");
  assert_eq!((fenced.refused, fenced.fenced.as_slice()), (1, [run].as_slice()));
  let state = work.run_state(run).expect("read").expect("a team run");
  assert_eq!(state.fence, Some(TeamFence::ClaimLost));
  assert_eq!(state.refused_reports, 1);

  // Fenced: nothing more is queued or sent, and the run, its inputs and its reports stay here.
  checkout.change_run(run, |run, now| run.resume(now));
  let sent = server.state().reports_sent;
  assert_eq!(work.report().await.expect("reported"), Reported::default());
  assert_eq!(server.state().reports_sent, sent);
  let db = checkout.db();
  assert_eq!(db.run(run).expect("read").expect("kept").status(), &RunStatus::Running);
  assert!(db.run_inputs(run).expect("read").is_some());
  let reports = db.work_reports(run).expect("reports");
  assert_eq!(
    reports.iter().map(|report| report.state).collect::<Vec<_>>(),
    [WorkReportState::Acknowledged, WorkReportState::Refused]
  );
  assert!(
    reports[1]
      .refusal
      .as_deref()
      .is_some_and(|refusal| refusal.contains("claim_lost"))
  );
}
