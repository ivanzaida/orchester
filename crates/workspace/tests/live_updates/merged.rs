//! A merged run's shared task ends Done on the team server (ORC-545): the merge's Done is sent before the run progress
//! report completes the claim it needs, whether the run change sends it first or the claim's end does; a status a
//! teammate set meanwhile stays theirs. Git may tell the claim's end that the run's work was merged before the merge's
//! own record is written, or without one, as for a merge made outside Orchester (ORC-750): the Done still goes first.
//!
//! `cargo test -p orchester-workspace --test live_updates merged_shared_task_done`

use std::{fs, path::Path, process};

use orchester_api::{TaskChanges, TaskPatch};
use orchester_core::{
  id::TaskId,
  integration::{MergeKind, RunMerge},
  revision::Revision,
  run::{BranchName, Run},
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

/// `git <args>` in `repo`, which must succeed.
fn git(repo: &Path, args: &[&str]) {
  let output = process::Command::new("git")
    .arg("-C")
    .arg(repo)
    .args(args)
    .output()
    .expect("run git");
  assert!(
    output.status.success(),
    "git {args:?}: {}",
    String::from_utf8_lossy(&output.stderr)
  );
}

/// The fixture's folder as a repository on `main` with one commit; that commit.
fn repository(fixture: &support::Fixture) -> String {
  let repo = fixture.repo.path();
  fs::remove_dir_all(repo.join(".git")).expect("the fixture's stand-in .git");
  git(repo, &["init", "--quiet", "--initial-branch=main"]);
  git(repo, &["config", "user.name", "Orchester Test"]);
  git(repo, &["config", "user.email", "test@orchester.invalid"]);
  git(repo, &["config", "commit.gpgsign", "false"]);
  fs::write(repo.join(".git").join("info").join("exclude"), "/.orchester/\n").expect("exclude .orchester");
  fs::write(repo.join("README.md"), "hello\n").expect("a file");
  git(repo, &["add", "--all"]);
  git(repo, &["commit", "--quiet", "-m", "initial"]);
  let head = process::Command::new("git")
    .arg("-C")
    .arg(repo)
    .args(["rev-parse", "HEAD"])
    .output()
    .expect("run git");
  String::from_utf8_lossy(&head.stdout).trim().to_owned()
}

/// Alice's desktop, whose run of the shared task `holder` holds, started from `main` of a real repository, passed;
/// the claim's holder is told of the run's In review.
async fn passed_in_git(server: &TeamServer, holder: &str) -> (Desktop, TaskId, Run) {
  let mut fixture = support::Fixture::new();
  let base = repository(&fixture);
  let task = fixture.task("Status bar", support::at(10));
  let mut run = fixture.started_run_from(task, (&base, "main"), support::at(11));
  let claimed = server.claimed(task, "Status bar", holder);
  let mut desktop = Desktop::signed_in(server, ALICE, fixture, &[claimed]);
  desktop.fixture.succeed(&mut run, support::at(12));
  desktop.servers.send_automatic_statuses().await;
  (desktop, task, run)
}

/// Merges `run`'s work, one commit on its branch, into `main` with git, as Orchester's merge or a person does.
fn merge_with_git(desktop: &Desktop, run: &Run) {
  let repo = desktop.root();
  git(repo, &["checkout", "--quiet", "-b", run.branch().as_str()]);
  fs::write(repo.join("status_bar.rs"), "work\n").expect("a file");
  git(repo, &["add", "--all"]);
  git(repo, &["commit", "--quiet", "-m", "status bar"]);
  git(repo, &["checkout", "--quiet", "main"]);
  git(
    repo,
    &["merge", "--quiet", "--no-ff", "-m", "merge", run.branch().as_str()],
  );
}

/// Alice's desktop, whose run of the shared task she holds, started from `main` of a real repository, passed, its In
/// review on the server, and whose work git merged into `main`: no merge of it is recorded here yet.
async fn merged_in_git(server: &TeamServer) -> (Desktop, TaskId, Run) {
  let (desktop, task, run) = passed_in_git(server, ALICE).await;
  assert_eq!(server.workflow(task), WorkflowStatus::InReview, "the run's In review");
  merge_with_git(&desktop, &run);
  assert_eq!(waiting(&desktop), 0, "no merge is recorded, so no Done waits yet");
  (desktop, task, run)
}

#[tokio::test(flavor = "multi_thread")]
async fn live_updates_merged_shared_task_done_when_the_claim_ends_before_the_merge_is_recorded() {
  let server = TeamServer::start(&[(ALICE, "Alice Moreau")], &[]);
  let (mut desktop, task, run) = merged_in_git(&server).await;

  // The claim's end reads the run merged from git while Orchester's merge still waits to record it.
  desktop.servers.end_claims(&Programs::processes()).await;
  assert!(!server.claimed_now(task), "the merge completed the claim");
  assert_eq!(server.workflow(task), WorkflowStatus::Done, "the Done went first");
  let merge = RunMerge {
    run: run.id(),
    into: BranchName::new("main").expect("a branch"),
    revision: Revision::new(&"b".repeat(40)).expect("a revision"),
    kind: MergeKind::MergeCommit,
    merged_at: support::at(13),
  };
  desktop.fixture.db.save_run_merge(&merge).expect("the merge");
  desktop.servers.send_automatic_statuses().await;

  assert_eq!(server.workflow(task), WorkflowStatus::Done);
  assert_eq!(waiting(&desktop), 0, "nothing waits");
  assert_eq!(unsent(&desktop, task), None, "nothing says it was not sent");
}

#[tokio::test(flavor = "multi_thread")]
async fn live_updates_merged_shared_task_done_when_git_alone_finds_the_merge() {
  let server = TeamServer::start(&[(ALICE, "Alice Moreau")], &[]);
  let (desktop, task, _run) = merged_in_git(&server).await;

  desktop.servers.end_claims(&Programs::processes()).await;
  desktop.servers.send_automatic_statuses().await;

  assert!(!server.claimed_now(task), "the merge completed the claim");
  assert_eq!(server.workflow(task), WorkflowStatus::Done);
  assert_eq!(waiting(&desktop), 0, "nothing waits");
  assert_eq!(unsent(&desktop, task), None, "nothing says it was not sent");
}

/// Bob, a teammate, moves `task` to In progress on the server.
async fn bob_moves(server: &TeamServer, task: TaskId) {
  let revision = server.revision(task);
  Member::of(server, BOB)
    .move_to(task, revision, WorkflowStatus::InProgress)
    .await;
}

fn assert_bobs_status_stays(server: &TeamServer, desktop: &Desktop, task: TaskId) {
  assert!(!server.claimed_now(task), "the merge completed the claim");
  assert_eq!(server.workflow(task), WorkflowStatus::InProgress, "Bob's status stays");
  let unsent = unsent(desktop, task).expect("the task says the Done was not sent");
  assert_eq!(
    unsent.reason,
    NotSent::Changed {
      current: WorkflowStatus::InProgress
    }
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn live_updates_merged_shared_task_done_found_by_git_is_not_sent_over_a_teammates_status() {
  let server = TeamServer::start(&[(ALICE, "Alice Moreau"), (BOB, "Bob Lind")], &[]);
  let (desktop, task, _run) = merged_in_git(&server).await;
  bob_moves(&server, task).await;

  // The sync caches Bob's status before it ends the claim.
  desktop.servers.sync_shared(&Programs::processes()).await;
  desktop.servers.send_automatic_statuses().await;

  assert_bobs_status_stays(&server, &desktop, task);
}

#[tokio::test(flavor = "multi_thread")]
async fn live_updates_merged_shared_task_done_not_sent_stays_so_when_the_claims_end_is_tried_again() {
  let server = TeamServer::start(&[(ALICE, "Alice Moreau"), (BOB, "Bob Lind")], &[]);
  let (mut desktop, task, run) = merged_in_git(&server).await;
  bob_moves(&server, task).await;
  server.refuse_run_reports(1);
  desktop.servers.end_claims(&Programs::processes()).await;
  assert!(server.claimed_now(task), "the refused report kept the claim");

  // Orchester's merge writes its record late, and the next claim's end tries again.
  let merge = RunMerge {
    run: run.id(),
    into: BranchName::new("main").expect("a branch"),
    revision: Revision::new(&"b".repeat(40)).expect("a revision"),
    kind: MergeKind::MergeCommit,
    merged_at: support::at(13),
  };
  desktop.fixture.db.save_run_merge(&merge).expect("the merge");
  desktop.servers.end_claims(&Programs::processes()).await;
  desktop.servers.send_automatic_statuses().await;

  assert_bobs_status_stays(&server, &desktop, task);
}

#[tokio::test(flavor = "multi_thread")]
async fn live_updates_merged_shared_task_done_is_not_sent_over_a_claim_another_member_holds() {
  let server = TeamServer::start(&[(ALICE, "Alice Moreau"), (BOB, "Bob Lind")], &[]);
  let (mut desktop, task, run) = passed_in_git(&server, BOB).await;
  let before = server.workflow(task);
  merge_with_git(&desktop, &run);

  desktop.servers.end_claims(&Programs::processes()).await;
  let merge = RunMerge {
    run: run.id(),
    into: BranchName::new("main").expect("a branch"),
    revision: Revision::new(&"b".repeat(40)).expect("a revision"),
    kind: MergeKind::MergeCommit,
    merged_at: support::at(13),
  };
  desktop.fixture.db.save_run_merge(&merge).expect("the merge");
  desktop.servers.send_automatic_statuses().await;

  assert!(server.claimed_now(task), "Bob's claim stays");
  assert_eq!(server.workflow(task), before, "the task's status stays the server's");
  let unsent = unsent(&desktop, task).expect("the task says the Done was not sent");
  assert_eq!(unsent.reason, NotSent::NotHolder);
}
