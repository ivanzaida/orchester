//! A merged run's shared task ends Done on the team server (ORC-545): the merge's Done is sent before the run progress
//! report completes the claim it needs, whether the run change sends it first or the claim's end does; a status a
//! teammate set meanwhile stays theirs.
//!
//! `cargo test -p orchester-workspace --test live_updates merged_shared_task_done`

use orchester_api::{TaskChanges, TaskPatch};
use orchester_core::{
  id::TaskId,
  integration::{MergeKind, RunMerge},
  revision::Revision,
  run::BranchName,
  sharing::TaskRevision,
  store::MergeStore,
  task::organization::WorkflowStatus,
};
use orchester_exec::programs::Programs;
use orchester_store::{NotSent, ProjectDb, UnsentStatus};

use crate::{ALICE, BOB, Member, desktop::Desktop, server::TeamServer, support};

impl Member {
  /// Moves `task`, read at `revision`, to `workflow`, as a person does.
  async fn move_to(&self, task: TaskId, revision: u64, workflow: WorkflowStatus) {
    let patch = TaskPatch {
      revision: TaskRevision::try_from(revision).expect("a revision"),
      changes: TaskChanges {
        workflow: Some(workflow),
        ..TaskChanges::default()
      },
    };
    let moved = self.client.update_task(&self.token, self.project, task, &patch).await;
    moved.expect("the move");
  }
}

/// Alice's desktop, whose run of the shared task she holds passed, its In review on the server, and was merged.
async fn merged(server: &TeamServer) -> (Desktop, TaskId) {
  let mut fixture = support::Fixture::new();
  let task = fixture.task("Status bar", support::at(10));
  let mut run = fixture.started_run(task, support::at(11));
  let claimed = server.claimed(task, "Status bar", ALICE);
  let mut desktop = Desktop::signed_in(server, ALICE, fixture, &[claimed]);
  desktop.fixture.succeed(&mut run, support::at(12));
  desktop.servers.send_automatic_statuses().await;
  assert_eq!(server.workflow(task), WorkflowStatus::InReview, "the run's In review");
  let merge = RunMerge {
    run: run.id(),
    into: BranchName::new("main").expect("a branch"),
    revision: Revision::new(&"b".repeat(40)).expect("a revision"),
    kind: MergeKind::FastForward,
    merged_at: support::at(13),
  };
  desktop.fixture.db.save_run_merge(&merge).expect("the merge");
  assert_eq!(waiting(&desktop), 1, "the merge's Done waits to be sent");
  (desktop, task)
}

fn waiting(desktop: &Desktop) -> usize {
  let db = ProjectDb::open(desktop.root()).expect("the project database");
  db.automatic_statuses().expect("the waiting statuses").len()
}

fn unsent(desktop: &Desktop, task: TaskId) -> Option<UnsentStatus> {
  let db = ProjectDb::open(desktop.root()).expect("the project database");
  db.unsent_status(task).expect("the unsent status")
}

#[tokio::test(flavor = "multi_thread")]
async fn live_updates_merged_shared_task_done_when_the_claim_ends_before_the_run_change_sends_it() {
  let server = TeamServer::start(&[(ALICE, "Alice Moreau")], &[]);
  let (desktop, task) = merged(&server).await;

  desktop.servers.end_claims(&Programs::processes()).await;
  assert!(!server.claimed_now(task), "the merge completed the claim");
  desktop.servers.send_automatic_statuses().await;

  assert_eq!(server.workflow(task), WorkflowStatus::Done);
  assert_eq!(waiting(&desktop), 0, "nothing waits");
  assert_eq!(unsent(&desktop, task), None, "nothing says it was not sent");
}

#[tokio::test(flavor = "multi_thread")]
async fn live_updates_merged_shared_task_done_when_the_run_change_sends_it_before_the_claim_ends() {
  let server = TeamServer::start(&[(ALICE, "Alice Moreau")], &[]);
  let (desktop, task) = merged(&server).await;

  desktop.servers.send_automatic_statuses().await;
  assert_eq!(server.workflow(task), WorkflowStatus::Done);
  desktop.servers.end_claims(&Programs::processes()).await;

  assert!(!server.claimed_now(task), "the merge completed the claim");
  assert_eq!(server.workflow(task), WorkflowStatus::Done);
  assert_eq!(waiting(&desktop), 0, "nothing waits");
  assert_eq!(unsent(&desktop, task), None, "nothing says it was not sent");
}

#[tokio::test(flavor = "multi_thread")]
async fn live_updates_merged_shared_task_done_is_not_sent_over_a_teammates_status() {
  let server = TeamServer::start(&[(ALICE, "Alice Moreau"), (BOB, "Bob Lind")], &[]);
  let (desktop, task) = merged(&server).await;
  let revision = server.revision(task);
  Member::of(&server, BOB)
    .move_to(task, revision, WorkflowStatus::InProgress)
    .await;

  desktop.servers.end_claims(&Programs::processes()).await;
  desktop.servers.send_automatic_statuses().await;

  assert!(!server.claimed_now(task), "the merge completed the claim");
  assert_eq!(server.workflow(task), WorkflowStatus::InProgress, "Bob's status stays");
  let unsent = unsent(&desktop, task).expect("the task says the Done was not sent");
  assert_eq!(
    unsent.reason,
    NotSent::Changed {
      current: WorkflowStatus::InProgress
    }
  );
}
