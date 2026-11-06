//! The heartbeat renews a claim only while its run works here (D1): nothing is sent for a paused or stopped run, the
//! server's `decision_required` is an outcome rather than a failure, its refusal for work it does not see running
//! renews nothing, and an authority it lists as lost fences the run.

use orchester_api::v2::DecisionId;
use orchester_core::run::RunLimit;
use orchester_store::TeamFence;
use orchester_workspace::team_work::Heartbeat;

use crate::{Checkout, started, team_server::TeamServer};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn team_work_heartbeat_renews_only_running_work() {
  let server = TeamServer::start(crate::fixture::Config::new("Append one line."));
  let checkout = Checkout::linked(&server);
  let (work, run, _generation) = started(&checkout, &server).await;
  let renewed = Heartbeat::Renewed {
    renewed: vec![run],
    lost: Vec::new(),
  };
  assert_eq!(work.heartbeat().await.expect("sent"), renewed);
  assert_eq!(server.state().heartbeats, 1);

  // The server says the work waits for a person: shown as such, and asked again only at the next heartbeat.
  server.wait_for_person(true);
  assert_eq!(
    work.heartbeat().await.expect("sent"),
    Heartbeat::DecisionRequired(vec![DecisionId::D1])
  );
  server.wait_for_person(false);

  // Paused or stopped at a limit here: nothing is sent.
  checkout.change_run(run, |run, now| run.request_pause(now));
  checkout.change_run(run, |run, now| run.pause(now));
  assert_eq!(work.heartbeat().await.expect("decided"), Heartbeat::NothingRunning);
  checkout.change_run(run, |run, now| run.resume(now));
  checkout.change_run(run, |run, now| run.stop_at_limit(RunLimit::DailyBudget, now));
  assert_eq!(work.heartbeat().await.expect("decided"), Heartbeat::NothingRunning);
  assert_eq!(server.state().heartbeats, 2, "no request for work that does not run");

  // The server saw the pause: until the run's report that it runs again reaches it, nothing is renewed.
  work.report().await.expect("reported");
  checkout.change_run(run, |run, now| run.resume(now));
  assert_eq!(work.heartbeat().await.expect("sent"), Heartbeat::NotRunning);
  work.report().await.expect("reported");
  assert_eq!(work.heartbeat().await.expect("sent"), renewed);

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
  assert_eq!(work.heartbeat().await.expect("decided"), Heartbeat::NothingRunning);
}
