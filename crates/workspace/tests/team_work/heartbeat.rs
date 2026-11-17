//! The heartbeat keeps a team task's claim while this computer holds work of it that the server sees running (OA-31):
//! a run at work, one whose step waits for a person and one a person paused are renewed alike. Work the server saw
//! pause at a limit or succeed is not named (the server renews none, D1), the server's `decision_required` is an
//! outcome rather than a failure, and an authority it lists as lost fences the run.

use orchester_api::v2::DecisionId;
use orchester_core::{
  run::{RunLimit, WaitReason},
  store::RunStore,
};
use orchester_store::TeamFence;
use orchester_workspace::team_work::Heartbeat;

use crate::{Checkout, claim_end::succeed, started, team_server::TeamServer};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn team_work_heartbeat_renews_the_claim_while_the_work_waits_or_is_paused_here() {
  let server = TeamServer::start(crate::fixture::Config::new("Append one line."));
  let checkout = Checkout::linked(&server);
  let (work, run, _generation) = started(&checkout, &server).await;
  let renewed = Heartbeat::Renewed {
    renewed: vec![run],
    lost: Vec::new(),
  };
  assert_eq!(work.heartbeat().await.expect("sent"), renewed);
  assert_eq!(server.state().heartbeats, 1);

  // A person paused it here: no report ends the server's work, and the claim is kept.
  checkout.change_run(run, |run, now| run.request_pause(now));
  checkout.change_run(run, |run, now| run.pause(now));
  work.report().await.expect("reported");
  assert_eq!(work.heartbeat().await.expect("sent"), renewed, "renewed while paused");
  checkout.change_run(run, |run, now| run.resume(now));

  // The server says the work waits for a person: shown as such, and asked again only at the next heartbeat.
  server.wait_for_person(true);
  assert_eq!(
    work.heartbeat().await.expect("sent"),
    Heartbeat::DecisionRequired(vec![DecisionId::D1])
  );
  server.wait_for_person(false);

  // Stopped at a limit and reported: the server paused the work and renews none, so nothing is named.
  checkout.change_run(run, |run, now| run.stop_at_limit(RunLimit::DailyBudget, now));
  work.report().await.expect("reported");
  let before = server.state().heartbeats;
  assert_eq!(work.heartbeat().await.expect("decided"), Heartbeat::NothingToRenew);
  assert_eq!(
    server.state().heartbeats,
    before,
    "no request for work the server paused"
  );
  checkout.change_run(run, |run, now| run.resume(now));
  work.report().await.expect("reported");
  assert_eq!(work.heartbeat().await.expect("sent"), renewed);

  // A step waits for a person's approval: the server's work goes on, and so does the claim.
  let execution = checkout.db().run(run).expect("read").expect("the run");
  let execution = execution.current_attempt().expect("attempt").executions()[0].id();
  checkout.change_run(run, |run, now| run.start_step(execution, now));
  checkout.change_run(run, |run, now| run.wait_step(execution, WaitReason::Approval, now));
  work.report().await.expect("reported");
  assert_eq!(work.heartbeat().await.expect("sent"), renewed, "renewed while it waits");

  // Offline: tried again at the next heartbeat.
  server.set_down(true);
  assert_eq!(work.heartbeat().await.expect("sent"), Heartbeat::Unreachable);
  server.set_down(false);

  // Taken over: the authority is lost and the run fenced; a fenced run renews nothing more.
  let claim = server.state().claim.clone().expect("held").id;
  crate::take_over(&server, claim).await;
  assert_eq!(
    work.heartbeat().await.expect("sent"),
    Heartbeat::Renewed {
      renewed: Vec::new(),
      lost: vec![run],
    }
  );
  let state = work.run_state(run).expect("read").expect("a team run");
  assert_eq!(state.fence, Some(TeamFence::ClaimLost));
  assert_eq!(work.heartbeat().await.expect("decided"), Heartbeat::NothingToRenew);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn team_work_heartbeat_names_no_work_the_server_saw_succeed() {
  let server = TeamServer::start(crate::fixture::Config::new("Append one line."));
  let checkout = Checkout::linked(&server);
  let (work, run, _generation) = started(&checkout, &server).await;
  succeed(&checkout, run);

  // Until its report reaches the server, the work runs there and its claim is renewed.
  let renewed = Heartbeat::Renewed {
    renewed: vec![run],
    lost: Vec::new(),
  };
  assert_eq!(work.heartbeat().await.expect("sent"), renewed);

  // Reported succeeded and waiting to be merged: the server renews no such claim (D1), so it is not named.
  work.report().await.expect("reported");
  let before = server.state().heartbeats;
  assert_eq!(work.heartbeat().await.expect("decided"), Heartbeat::NothingToRenew);
  assert_eq!(server.state().heartbeats, before);
}
