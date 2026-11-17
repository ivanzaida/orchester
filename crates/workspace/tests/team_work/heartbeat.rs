//! The heartbeat keeps a team task's claim while this computer holds work of it (OA-31): a run at work, one whose step
//! waits for a person, one a person paused, one the server saw pause at a limit, and one it saw succeed until its
//! merge here is answered with the task's Done; and a module's claim while its delivery goes on. The server's
//! `decision_required` is an outcome rather than a failure, an authority it lists as lost fences the run, and a server
//! older than this renewal is sent the running claims alone.

use orchester_api::v2::DecisionId;
use orchester_core::{
  run::{RunLimit, WaitReason},
  store::RunStore,
};
use orchester_store::TeamFence;
use orchester_workspace::team_work::{DeliveryProgress, Heartbeat};

use crate::{
  Checkout, StoreRunner,
  claim_end::succeed,
  delivery::{claim_module, person_deliveries, repository, request},
  done::merge,
  started,
  team_server::{MEMBER, TeamServer},
};

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

  // Stopped at a limit and reported: the server paused the work, and the claim is kept while the run may go on.
  checkout.change_run(run, |run, now| run.stop_at_limit(RunLimit::DailyBudget, now));
  work.report().await.expect("reported");
  assert_eq!(work.heartbeat().await.expect("sent"), renewed, "renewed at a limit");
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
async fn team_work_heartbeat_keeps_succeeded_work_until_its_merge_sets_the_task_done() {
  let server = TeamServer::start(crate::fixture::Config::new("Append one line."));
  let checkout = Checkout::linked(&server);
  let (work, run, _generation) = started(&checkout, &server).await;
  succeed(&checkout, run);
  let renewed = Heartbeat::Renewed {
    renewed: vec![run],
    lost: Vec::new(),
  };
  assert_eq!(work.heartbeat().await.expect("sent"), renewed);

  // Reported succeeded and waiting to be merged: the claim is kept.
  work.report().await.expect("reported");
  assert_eq!(work.heartbeat().await.expect("sent"), renewed, "renewed in review");

  // Merged here, its Done not answered yet: still kept.
  merge(&checkout, run);
  assert_eq!(work.heartbeat().await.expect("sent"), renewed, "renewed until Done");

  // The Done released the claim: nothing is named any more.
  work.send_merged_done().await.expect("sent");
  let before = server.state().heartbeats;
  assert_eq!(work.heartbeat().await.expect("decided"), Heartbeat::NothingToRenew);
  assert_eq!(server.state().heartbeats, before, "no request once the task is Done");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn team_work_heartbeat_an_older_server_renews_the_running_claims_alone() {
  let server = TeamServer::start(crate::fixture::Config::new("Append one line."));
  server.renew_running_only();
  let checkout = Checkout::linked(&server);
  let (base, head) = repository(&checkout);
  let module_claim = claim_module(&checkout, &server).await;
  let (deliveries, _deliveries_generation) = person_deliveries(&checkout, &server);
  let waits = deliveries
    .request(request(&server, module_claim, (&base, &head), None))
    .await
    .expect("carried on");
  assert!(matches!(waits, DeliveryProgress::AwaitingAdmin(_)), "{waits:?}");

  // Only the module's claim: the older server refuses it, and there is nothing running to name alone.
  let (alone, _alone_generation) = checkout.work(&server, MEMBER, StoreRunner::new(checkout.root(), &[]));
  assert_eq!(
    alone.heartbeat().await.expect("answered"),
    Heartbeat::DecisionRequired(vec![DecisionId::D1])
  );

  // Running work beside it: refused, then the running claim is sent again alone and renewed.
  let (work, run, _generation) = started(&checkout, &server).await;
  let before = server.state().heartbeats;
  assert_eq!(
    work.heartbeat().await.expect("sent"),
    Heartbeat::Renewed {
      renewed: vec![run],
      lost: Vec::new(),
    }
  );
  assert_eq!(
    server.state().heartbeats,
    before + 2,
    "refused, then the running claim alone"
  );
  assert_eq!(server.state().module_renewals, 0);

  // Succeeded and reported: kept work the older server refuses, with nothing running left.
  succeed(&checkout, run);
  work.report().await.expect("reported");
  assert_eq!(work.heartbeat().await.expect("answered"), Heartbeat::NotRunning);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn team_work_heartbeat_keeps_the_module_claim_while_its_delivery_waits() {
  let server = TeamServer::start(crate::fixture::Config::new("Append one line."));
  let checkout = Checkout::linked(&server);
  let (base, head) = repository(&checkout);
  let claim = claim_module(&checkout, &server).await;
  let (work, _generation) = checkout.work(&server, MEMBER, StoreRunner::new(checkout.root(), &[]));
  assert_eq!(
    work.heartbeat().await.expect("decided"),
    Heartbeat::NothingToRenew,
    "no delivery asked for yet"
  );

  let (deliveries, _deliveries_generation) = person_deliveries(&checkout, &server);
  let waits = deliveries
    .request(request(&server, claim, (&base, &head), None))
    .await
    .expect("carried on");
  assert!(matches!(waits, DeliveryProgress::AwaitingAdmin(_)), "{waits:?}");
  let none = Heartbeat::Renewed {
    renewed: Vec::new(),
    lost: Vec::new(),
  };
  assert_eq!(work.heartbeat().await.expect("sent"), none);
  assert_eq!(server.state().module_renewals, 1, "the module's claim is renewed");

  // The claim ended on the server: it is lost, and the next heartbeat names it again only while the delivery waits.
  server.state().module_claim = None;
  assert_eq!(work.heartbeat().await.expect("sent"), none);
  assert_eq!(server.state().module_renewals, 1);
}
