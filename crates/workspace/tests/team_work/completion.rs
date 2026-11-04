//! A team run's success and merge are its own records: they never change its task here, and never make it Done (D4
//! is open), whose status the team server keeps.

use orchester_core::{
  integration::{MergeKind, RunMerge},
  revision::Revision,
  run::{BranchName, RunStatus},
  store::{MergeStore, RunStore, TaskStore},
  time::Timestamp,
};

use crate::{Checkout, started, team_server::TeamServer};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn team_work_a_succeeded_and_merged_team_run_changes_no_task_here() {
  let server = TeamServer::start(crate::fixture::Config::new("Append one line."));
  let checkout = Checkout::linked(&server);
  let (_work, run, _generation) = started(&checkout, &server).await;
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
  let mut db = checkout.db();
  let task = db.run(run).expect("read").expect("the run").task();
  assert_eq!(
    db.run(run).expect("read").expect("the run").status(),
    &RunStatus::Succeeded
  );

  let merge = RunMerge {
    run,
    into: BranchName::new("module/webhooks").expect("branch"),
    revision: Revision::new(&"b".repeat(40)).expect("revision"),
    kind: MergeKind::FastForward,
    merged_at: Timestamp::now(),
  };
  db.save_run_merge(&merge).expect("the merge is recorded");
  assert!(
    db.task(task).expect("read").is_none(),
    "no definition of the team task here"
  );
  assert!(
    db.automatic_statuses().expect("statuses").is_empty(),
    "no status waits to be sent for it"
  );
}
