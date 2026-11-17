//! A team task is Done once its run's work is merged here, as a local task is (OA-30): the merge records the Done, and
//! the holder sets it on the team server, which releases the claim. A server that cannot be reached keeps it waiting;
//! a refusal is kept with the run, never sent again and never dropped.

use orchester_core::{
  id::RunId,
  integration::{MergeKind, RunMerge},
  revision::Revision,
  run::BranchName,
  store::MergeStore,
  time::Timestamp,
};
use orchester_store::MergedDoneState;
use orchester_workspace::team_work::DoneSent;

use crate::{
  Checkout,
  claim_end::succeed,
  started,
  team_server::{MODULE_BRANCH, TeamServer},
};

/// The run's work merged into its module's branch, as Orchester records it.
pub(crate) fn merge(checkout: &Checkout, run: RunId) {
  let merge = RunMerge {
    run,
    into: BranchName::new(MODULE_BRANCH).expect("branch"),
    revision: Revision::new(&"b".repeat(40)).expect("revision"),
    kind: MergeKind::FastForward,
    merged_at: Timestamp::now(),
  };
  checkout.db().save_run_merge(&merge).expect("the merge is recorded");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn team_work_done_a_merged_team_run_sets_its_task_done_on_the_server() {
  let server = TeamServer::start(crate::fixture::Config::new("Append one line."));
  let checkout = Checkout::linked(&server);
  let (work, run, _generation) = started(&checkout, &server).await;
  succeed(&checkout, run);
  work.report().await.expect("reported");
  assert_eq!(
    work.send_merged_done().await.expect("looked"),
    DoneSent::default(),
    "nothing merged yet"
  );

  merge(&checkout, run);
  let done =
    |work: &orchester_workspace::team_work::TeamWork<_>| work.run_state(run).expect("read").expect("a team run").done;
  assert_eq!(done(&work), Some(MergedDoneState::Pending));

  // The server cannot be reached: the Done waits.
  server.set_down(true);
  let waiting = work.send_merged_done().await.expect("looked");
  assert!(waiting.waiting, "{waiting:?}");
  assert_eq!(done(&work), Some(MergedDoneState::Pending));
  server.set_down(false);

  let sent = work.send_merged_done().await.expect("sent");
  assert_eq!(sent.sent, vec![run]);
  {
    let state = server.state();
    assert_eq!(state.workflow, "done");
    assert!(state.claim.is_none(), "closing released the claim");
    assert_eq!(state.task_patches, 1);
  }
  assert_eq!(done(&work), Some(MergedDoneState::Sent));

  assert_eq!(work.send_merged_done().await.expect("looked"), DoneSent::default());
  assert_eq!(server.state().task_patches, 1, "sent once");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn team_work_done_a_task_done_on_the_server_already_is_not_patched() {
  let server = TeamServer::start(crate::fixture::Config::new("Append one line."));
  let checkout = Checkout::linked(&server);
  let (work, run, _generation) = started(&checkout, &server).await;
  succeed(&checkout, run);
  work.report().await.expect("reported");
  merge(&checkout, run);
  "done".clone_into(&mut server.state().workflow);

  assert_eq!(work.send_merged_done().await.expect("looked").sent, vec![run]);
  assert_eq!(server.state().task_patches, 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn team_work_done_a_refusal_is_kept_with_the_run() {
  let server = TeamServer::start(crate::fixture::Config::new("Append one line."));
  let checkout = Checkout::linked(&server);
  let (work, run, _generation) = started(&checkout, &server).await;
  succeed(&checkout, run);
  work.report().await.expect("reported");
  let claim = server.state().claim.clone().expect("held").id;
  crate::take_over(&server, claim).await;
  merge(&checkout, run);

  let not_holder = "not_holder".to_owned();
  let sent = work.send_merged_done().await.expect("answered");
  assert_eq!(sent.not_sent, vec![(run, not_holder.clone())]);
  assert_eq!(server.state().workflow, "todo", "the server's status stands");
  let state = work.run_state(run).expect("read").expect("a team run");
  assert_eq!(state.done, Some(MergedDoneState::NotSent { reason: not_holder }));

  assert_eq!(work.send_merged_done().await.expect("looked"), DoneSent::default());
  assert_eq!(server.state().task_patches, 0, "a refusal is not sent again");
}
