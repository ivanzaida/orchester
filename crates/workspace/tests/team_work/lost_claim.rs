//! A team run whose claim someone else took over stops (OA-31): once the server says its authority is gone, the run is
//! fenced, reports nothing more, and is cancelled here, its output staying on this computer; the fence says why.

use orchester_core::{run::RunStatus, store::RunStore};
use orchester_store::TeamFence;
use orchester_workspace::team_work::{Heartbeat, StartOutcome};

use crate::{
  Checkout, StoreRunner,
  fixture::Config,
  take_over,
  team_server::{MEMBER, TeamServer},
};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn team_work_lost_claim_stops_the_run_and_its_reports() {
  let server = TeamServer::start(Config::new("Append one line."));
  let checkout = Checkout::linked(&server);
  let claim = checkout.claim(&server, MEMBER).await;
  let runner = StoreRunner::new(checkout.root(), &[]);
  let (work, _generation) = checkout.work(&server, MEMBER, runner.clone());
  let outcome = checkout
    .start(&work, checkout.start_request(&server, claim))
    .await
    .expect("started");
  let StartOutcome::Started(run) = outcome else {
    panic!("not started: {outcome:?}");
  };
  work.report().await.expect("reported");
  assert!(
    work.stop_fenced().await.expect("looked").is_empty(),
    "nothing is fenced"
  );

  take_over(&server, claim).await;
  assert_eq!(
    work.heartbeat().await.expect("sent"),
    Heartbeat::Renewed {
      renewed: Vec::new(),
      lost: vec![run],
    }
  );
  assert_eq!(work.stop_fenced().await.expect("stopped"), vec![run]);
  assert_eq!(runner.cancelled(), 1);
  let stopped = checkout.db().run(run).expect("read").expect("the run");
  assert_eq!(stopped.status(), &RunStatus::Cancelled);

  // Fenced: its cancellation is not reported, and the fence tells why it stopped.
  let sent = server.state().reports_sent;
  work.report().await.expect("looked");
  assert_eq!(server.state().reports_sent, sent, "a fenced run reports nothing");
  let state = work.run_state(run).expect("read").expect("a team run");
  assert_eq!(state.fence, Some(TeamFence::ClaimLost));
  assert!(work.stop_fenced().await.expect("looked").is_empty(), "stopped once");
  assert_eq!(runner.cancelled(), 1);
}
