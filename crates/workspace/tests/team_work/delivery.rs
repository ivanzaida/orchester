//! Module delivery (team authority P4c) against the fake server's delivery commands, in a real repository whose
//! checkout is on `main` and whose module branch is one commit ahead of it: a request without an authorization waits
//! for the Admin's, and with it the merge lands and is reported; the automatic merge authorizes on its own, and its
//! report waits across an outage; nothing is sent while the module's work runs here, and the server refuses a second
//! request while one is not reported; a lost answer is looked up, not sent again; and a merge made before the computer
//! stopped is found on recovery, reported once, never made twice.

use std::{fs, path::Path, process::Command};

use orchester_api::v2::{AuthorityErrorCode, DeliveryState, Subject, TeamCommandValue};
use orchester_core::{
  id::{ClaimId, OperationId},
  run::{BranchName, CommitId},
};
use orchester_exec::{local_git::IntegrationLease, programs::Programs};
use orchester_store::{DeliveryMerge, ProjectDb, TeamDeliveryStage};
use orchester_workspace::{
  actions::{Accepted, Action, ActionOutcome, ClaimAction, DeliveryAction, DeliveryStep, Intent, TeamOutcome},
  team_work::{DeliveryProgress, DeliveryStatus, ModuleDeliveries, TeamDeliveryRequest, TeamDeliveryState},
};
use tokio::sync::watch;

use crate::{
  Checkout, started,
  team_server::{ADMIN, MEMBER, MODULE_BRANCH, TeamServer},
};

pub(crate) fn git(root: &Path, args: &[&str]) -> String {
  let output = Command::new("git").args(args).current_dir(root).output().expect("git");
  assert!(
    output.status.success(),
    "git {args:?}: {}",
    String::from_utf8_lossy(&output.stderr)
  );
  String::from_utf8(output.stdout).expect("text").trim().to_owned()
}

pub(crate) fn commit(sha: &str) -> CommitId {
  CommitId::new(sha).expect("a full commit id")
}

/// The checkout made a repository on `main`, with the module's branch one commit ahead of it, as a task run merged
/// into it left it (D3): the base commit and the module branch's head.
pub(crate) fn repository(checkout: &Checkout) -> (CommitId, CommitId) {
  let root = checkout.root();
  fs::remove_dir_all(root.join(".git")).expect("the placeholder");
  git(root, &["init", "-b", "main"]);
  git(root, &["config", "user.name", "Delivery test"]);
  git(root, &["config", "user.email", "test@example.invalid"]);
  git(root, &["config", "core.autocrlf", "false"]);
  orchester_store::exclude_project_data(root).expect("excluded");
  fs::write(root.join("app.txt"), "base\n").expect("app");
  git(root, &["add", "app.txt"]);
  git(root, &["commit", "-m", "base"]);
  let base = git(root, &["rev-parse", "HEAD"]);
  git(root, &["switch", "-c", MODULE_BRANCH]);
  fs::write(root.join("payments.txt"), "payments\n").expect("payments");
  git(root, &["add", "payments.txt"]);
  git(root, &["commit", "-m", "Merge KON-7 run #1"]);
  let head = git(root, &["rev-parse", "HEAD"]);
  git(root, &["switch", "main"]);
  (commit(&base), commit(&head))
}

/// The module's claim, acquired in the person's session.
pub(crate) async fn claim_module(checkout: &Checkout, server: &TeamServer) -> ClaimId {
  let (dispatcher, _generation) = checkout.dispatcher(server, MEMBER);
  let acquire = Action::Claim(ClaimAction::Acquire {
    subject: Subject::Module { id: server.module },
  });
  let outcome = dispatcher.dispatch(&Intent::new(acquire)).await.expect("dispatched");
  let ActionOutcome::Team(TeamOutcome::Completed(Accepted::Command(receipt))) = outcome else {
    panic!("not claimed: {outcome:?}");
  };
  let TeamCommandValue::Claim { claim, .. } = receipt.result else {
    panic!("not a claim: {receipt:?}");
  };
  claim.id
}

/// The person's module deliveries, and the session's generation they keep.
pub(crate) fn person_deliveries(checkout: &Checkout, server: &TeamServer) -> (ModuleDeliveries, watch::Sender<u64>) {
  let (dispatcher, generation) = checkout.dispatcher(server, MEMBER);
  (ModuleDeliveries::new(dispatcher, Programs::processes()), generation)
}

fn delivery(
  server: &TeamServer,
  claim: ClaimId,
  (base, head): (&CommitId, &CommitId),
  step: DeliveryStep,
) -> DeliveryAction {
  DeliveryAction {
    module: server.module,
    claim,
    branch: BranchName::new(MODULE_BRANCH).expect("branch"),
    source_commit: head.clone(),
    base_commit: base.clone(),
    step,
  }
}

/// The holder's request of the module branch's head onto `main` at `base`.
pub(crate) fn request(
  server: &TeamServer,
  claim: ClaimId,
  commits: (&CommitId, &CommitId),
  authorization: Option<OperationId>,
) -> TeamDeliveryRequest {
  TeamDeliveryRequest {
    operation: OperationId::generate(),
    delivery: delivery(server, claim, commits, DeliveryStep::Request { authorization }),
    base_branch: BranchName::new("main").expect("branch"),
  }
}

/// An Admin authorizes the delivery from their own computer; the operation the holder cites.
async fn authorize(server: &TeamServer, claim: ClaimId, commits: (&CommitId, &CommitId)) -> OperationId {
  let admin = Checkout::linked(server);
  let (dispatcher, _generation) = admin.dispatcher(server, ADMIN);
  let intent = Intent::new(Action::Delivery(delivery(
    server,
    claim,
    commits,
    DeliveryStep::Authorize,
  )));
  let outcome = dispatcher.dispatch(&intent).await.expect("dispatched");
  let ActionOutcome::Team(TeamOutcome::Completed(Accepted::Command(receipt))) = outcome else {
    panic!("not authorized: {outcome:?}");
  };
  let TeamCommandValue::Delivery { delivery, effects } = receipt.result else {
    panic!("not a delivery: {receipt:?}");
  };
  assert_eq!(delivery.state, DeliveryState::Authorized);
  assert!(effects.is_empty(), "the Admin's computer merges nothing");
  intent.operation()
}

pub(crate) fn state(deliveries: &ModuleDeliveries, operation: OperationId) -> TeamDeliveryState {
  deliveries.delivery_state(operation).expect("read").expect("a delivery")
}

/// The merge commits on `main`.
pub(crate) fn merges(root: &Path) -> Vec<String> {
  git(root, &["rev-list", "--merges", "main"])
    .lines()
    .map(str::to_owned)
    .collect()
}

/// `main`'s tip, which must be one merge commit of exactly `base` and `head`.
pub(crate) fn merged_tip(root: &Path, (base, head): (&CommitId, &CommitId)) -> String {
  let tip = git(root, &["rev-parse", "main"]);
  assert_eq!(
    git(root, &["rev-list", "--parents", "-n", "1", &tip]),
    format!("{tip} {} {}", base.as_str(), head.as_str()),
    "a merge commit of the authorized commits"
  );
  assert_eq!(merges(root).len(), 1, "merged once");
  tip
}

pub(crate) fn delivered(progress: DeliveryProgress) -> orchester_api::v2::ModuleDeliveryRead {
  let DeliveryProgress::Reported(delivery) = progress else {
    panic!("not reported: {progress:?}");
  };
  assert_eq!(delivery.state, DeliveryState::Delivered);
  delivery
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn team_work_delivery_request_then_admin_authorize_then_merge_then_report() {
  let server = TeamServer::start(crate::fixture::Config::new("Append one line."));
  let checkout = Checkout::linked(&server);
  let (base, head) = repository(&checkout);
  let claim = claim_module(&checkout, &server).await;
  let (deliveries, _generation) = person_deliveries(&checkout, &server);

  let unauthorized = request(&server, claim, (&base, &head), None);
  let refused = deliveries.request(unauthorized.clone()).await.expect("carried on");
  assert!(
    matches!(&refused, DeliveryProgress::Refused(error) if error.code() == AuthorityErrorCode::DeliveryNotAuthorized),
    "{refused:?}"
  );
  assert_eq!(
    state(&deliveries, unauthorized.operation).status,
    DeliveryStatus::AwaitingAuthorization
  );
  assert!(
    merges(checkout.root()).is_empty(),
    "nothing merged without an authorization"
  );

  let authorization = authorize(&server, claim, (&base, &head)).await;
  let authorized = request(&server, claim, (&base, &head), Some(authorization));
  let delivery = delivered(deliveries.request(authorized.clone()).await.expect("carried on"));
  assert_eq!(delivery.authorization_operation_id, Some(authorization));
  let tip = merged_tip(checkout.root(), (&base, &head));
  assert_eq!(
    fs::read_to_string(checkout.root().join("payments.txt")).expect("in the checkout"),
    "payments\n"
  );
  assert_eq!(server.state().deliveries[0].merge_commit.as_deref(), Some(tip.as_str()));
  let state = state(&deliveries, authorized.operation);
  assert_eq!(state.status, DeliveryStatus::Delivered);
  assert_eq!(state.merge, Some(DeliveryMerge::Merged { commit: commit(&tip) }));
  assert!(!state.report_waiting);
  let (dispatcher, _generation) = checkout.dispatcher(&server, MEMBER);
  assert!(
    dispatcher.open_operations().expect("read").is_empty(),
    "the request's effect is carried out and its operation completed"
  );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn team_work_delivery_automatic_merge_authorizes_and_its_report_waits_out_an_outage() {
  let server = TeamServer::start(crate::fixture::Config::new("Append one line."));
  let checkout = Checkout::linked(&server);
  let (base, head) = repository(&checkout);
  let claim = claim_module(&checkout, &server).await;
  server.state().automatic_merge = true;
  let (deliveries, _generation) = person_deliveries(&checkout, &server);

  // Another merge holds the checkout: the request is taken, the merge waits.
  let lease = IntegrationLease::acquire(checkout.root()).expect("the checkout held");
  let automatic = request(&server, claim, (&base, &head), None);
  let progress = deliveries.request(automatic.clone()).await.expect("carried on");
  assert!(matches!(progress, DeliveryProgress::MergeWaiting), "{progress:?}");
  assert_eq!(
    state(&deliveries, automatic.operation).status,
    DeliveryStatus::AuthorizedAutomatically
  );
  drop(lease);

  // The server goes away after the merge: the report is kept and waits.
  server.set_down(true);
  let recovered = deliveries.recover().await.expect("recovered");
  assert!(
    matches!(
      recovered.as_slice(),
      [(_, Ok(DeliveryProgress::Waiting(TeamDeliveryStage::Reporting)))]
    ),
    "{recovered:?}"
  );
  let tip = merged_tip(checkout.root(), (&base, &head));
  let waiting = state(&deliveries, automatic.operation);
  assert_eq!(waiting.status, DeliveryStatus::AuthorizedAutomatically);
  assert_eq!(waiting.merge, Some(DeliveryMerge::Merged { commit: commit(&tip) }));
  assert!(waiting.report_waiting);
  assert_eq!(server.state().delivery_reports, 0);

  server.set_down(false);
  let recovered = deliveries.recover().await.expect("recovered");
  let [(operation, Ok(progress))] = <[_; 1]>::try_from(recovered).expect("one delivery") else {
    panic!("not carried on");
  };
  assert_eq!(operation, automatic.operation);
  let delivery = delivered(progress);
  assert_eq!(
    delivery.authorization_operation_id, None,
    "the automatic merge authorized it"
  );
  assert_eq!(
    state(&deliveries, automatic.operation).status,
    DeliveryStatus::Delivered
  );
  assert_eq!(server.state().delivery_reports, 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn team_work_delivery_refused_while_work_runs() {
  let server = TeamServer::start(crate::fixture::Config::new("Append one line."));
  let checkout = Checkout::linked(&server);
  let (base, head) = repository(&checkout);
  let (_work, run, _work_generation) = started(&checkout, &server).await;
  let claim = claim_module(&checkout, &server).await;
  server.state().automatic_merge = true;
  let (deliveries, _generation) = person_deliveries(&checkout, &server);

  let early = request(&server, claim, (&base, &head), None);
  let progress = deliveries.request(early).await.expect("carried on");
  assert!(
    matches!(&progress, DeliveryProgress::WorkRunning(runs) if *runs == [run]),
    "{progress:?}"
  );
  assert_eq!(server.state().delivery_requests, 0, "nothing sent");
  assert!(deliveries.deliveries().expect("read").is_empty(), "nothing recorded");

  checkout.change_run(run, |run, now| run.cancel(now));
  let lease = IntegrationLease::acquire(checkout.root()).expect("the checkout held");
  let first = request(&server, claim, (&base, &head), None);
  let progress = deliveries.request(first.clone()).await.expect("carried on");
  assert!(matches!(progress, DeliveryProgress::MergeWaiting), "{progress:?}");

  // The server refuses another request of the claim while the first is not reported.
  let second = request(&server, claim, (&base, &head), None);
  let progress = deliveries.request(second.clone()).await.expect("carried on");
  assert!(
    matches!(&progress, DeliveryProgress::Refused(error) if error.code() == AuthorityErrorCode::TransitionDenied),
    "{progress:?}"
  );
  assert!(matches!(
    state(&deliveries, second.operation).status,
    DeliveryStatus::Refused(error) if error.code() == AuthorityErrorCode::TransitionDenied
  ));
  drop(lease);

  let recovered = deliveries.recover().await.expect("recovered");
  assert!(
    matches!(recovered.as_slice(), [(operation, Ok(DeliveryProgress::Reported(_)))] if *operation == first.operation),
    "only the first is open: {recovered:?}"
  );
  merged_tip(checkout.root(), (&base, &head));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn team_work_delivery_a_lost_response_is_retried_safely() {
  let server = TeamServer::start(crate::fixture::Config::new("Append one line."));
  let checkout = Checkout::linked(&server);
  let (base, head) = repository(&checkout);
  let claim = claim_module(&checkout, &server).await;
  server.state().automatic_merge = true;
  let (deliveries, _generation) = person_deliveries(&checkout, &server);

  server.lose_next_response();
  let lost = request(&server, claim, (&base, &head), None);
  let progress = deliveries.request(lost.clone()).await.expect("carried on");
  assert!(
    matches!(progress, DeliveryProgress::Waiting(TeamDeliveryStage::Requested)),
    "{progress:?}"
  );
  assert_eq!(state(&deliveries, lost.operation).status, DeliveryStatus::Requested);
  assert!(merges(checkout.root()).is_empty(), "no merge on an answer not read");

  // Sent again, the request is looked up; the report's answer is lost in turn and the report is sent again.
  server.lose_next_response();
  let progress = deliveries.request(lost.clone()).await.expect("carried on");
  assert!(
    matches!(progress, DeliveryProgress::Waiting(TeamDeliveryStage::Reporting)),
    "{progress:?}"
  );
  delivered(deliveries.request(lost.clone()).await.expect("carried on"));
  merged_tip(checkout.root(), (&base, &head));
  let server = server.state();
  assert_eq!(
    (server.delivery_requests, server.delivery_reports),
    (1, 1),
    "each carried out once"
  );
  assert_eq!(server.deliveries.len(), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn team_work_delivery_a_crash_between_merge_and_report_recovers_once() {
  let server = TeamServer::start(crate::fixture::Config::new("Append one line."));
  let checkout = Checkout::linked(&server);
  let (base, head) = repository(&checkout);
  let claim = claim_module(&checkout, &server).await;
  server.state().automatic_merge = true;
  let (deliveries, generation) = person_deliveries(&checkout, &server);

  let lease = IntegrationLease::acquire(checkout.root()).expect("the checkout held");
  let crashed = request(&server, claim, (&base, &head), None);
  let progress = deliveries.request(crashed.clone()).await.expect("carried on");
  assert!(matches!(progress, DeliveryProgress::MergeWaiting), "{progress:?}");
  let stage = ProjectDb::open(checkout.root())
    .expect("open")
    .team_delivery(crashed.operation)
    .expect("read")
    .expect("recorded")
    .stage;
  assert_eq!(stage, TeamDeliveryStage::Merging, "marked before git was asked");
  // Git made the merge as the delivery makes it, and the computer stopped before it was recorded.
  git(
    checkout.root(),
    &[
      "merge",
      "--no-ff",
      "--no-edit",
      "-m",
      "Merge module/payments",
      MODULE_BRANCH,
    ],
  );
  let tip = merged_tip(checkout.root(), (&base, &head));
  drop((lease, deliveries, generation));

  let (deliveries, _generation) = person_deliveries(&checkout, &server);
  let recovered = deliveries.recover().await.expect("recovered");
  let [(_, Ok(progress))] = <[_; 1]>::try_from(recovered).expect("one delivery") else {
    panic!("not carried on");
  };
  delivered(progress);
  assert_eq!(merged_tip(checkout.root(), (&base, &head)), tip, "not merged again");
  assert_eq!(server.state().deliveries[0].merge_commit.as_deref(), Some(tip.as_str()));
  assert_eq!(server.state().delivery_reports, 1);
  assert!(
    matches!(deliveries.recover().await.expect("recovered").as_slice(), []),
    "nothing left to carry on"
  );
}
