//! The claims of runs that ended are ended with care for the team server's limit (ORC-896): a team run's claim is
//! settled by its work reports, so it gets no claim end here; a claim end the server answered `409 Conflict` is not
//! tried again; and a burst of run changes ends claims no more often than the minute's round does.
//!
//! `cargo test -p orchester-workspace --test live_updates live_updates_claim_ends`

use std::time::Duration;

use orchester_core::{
  id::{ClaimId, OperationId, RunId, TaskId, WorkId},
  store::RunStore,
};
use orchester_exec::programs::Programs;
use orchester_store::ProjectDb;
use tokio::sync::watch;

use crate::{ALICE, desktop::Desktop, merged::merged, server::TeamServer};

/// The latest run of `task` here.
fn latest_run(desktop: &Desktop, task: TaskId) -> RunId {
  let db = ProjectDb::open(desktop.root()).expect("the project database");
  db.runs_of_task(task)
    .expect("the runs")
    .pop()
    .expect("a run")
    .expect("a readable run")
    .id()
}

/// Records `run` as a team run, as a start through the team server does; only the row the claim ends look at.
fn make_team_run(desktop: &Desktop, run: RunId) {
  let database =
    rusqlite::Connection::open(desktop.root().join(".orchester/orchester.db")).expect("open the project database");
  // The start and the claim the row names are not there: only the run is.
  database
    .pragma_update(None, "foreign_keys", false)
    .expect("foreign keys off");
  database
    .execute(
      "INSERT INTO team_runs (run_id, operation_id, team_task_id, claim_id, work_id, next_sequence) \
       VALUES (?1, ?2, ?3, ?4, ?5, 1)",
      [
        run.to_string(),
        OperationId::generate().to_string(),
        TaskId::generate().to_string(),
        ClaimId::generate().to_string(),
        WorkId::generate().to_string(),
      ],
    )
    .expect("a team run");
}

#[tokio::test(flavor = "multi_thread")]
async fn live_updates_claim_ends_a_team_runs_ended_claim_gets_no_claim_end() {
  let server = TeamServer::start(&[(ALICE, "Alice Moreau")], &[]);
  let (desktop, task) = merged(&server).await;
  make_team_run(&desktop, latest_run(&desktop, task));

  desktop.servers.end_claims(&Programs::processes()).await;

  assert_eq!(server.run_reports(), 0, "no run progress report was sent");
  assert!(server.claimed_now(task), "the claim is the work reports' to settle");
}

#[tokio::test(flavor = "multi_thread")]
async fn live_updates_claim_ends_a_conflict_is_not_tried_again() {
  let server = TeamServer::start(&[(ALICE, "Alice Moreau")], &[]);
  let (desktop, task) = merged(&server).await;
  server.conflict_run_reports(true);

  desktop.servers.end_claims(&Programs::processes()).await;
  assert_eq!(server.run_reports(), 1, "the claim's end was tried and refused");

  server.conflict_run_reports(false);
  desktop.servers.end_claims(&Programs::processes()).await;
  desktop.servers.end_claims(&Programs::processes()).await;

  assert_eq!(server.run_reports(), 1, "no second request");
  assert!(server.claimed_now(task), "the claim stays as the server has it");
}

#[tokio::test(flavor = "multi_thread")]
async fn live_updates_claim_ends_a_failure_other_than_a_conflict_is_tried_again() {
  let server = TeamServer::start(&[(ALICE, "Alice Moreau")], &[]);
  let (desktop, task) = merged(&server).await;
  server.refuse_run_reports(1);

  desktop.servers.end_claims(&Programs::processes()).await;
  assert!(server.claimed_now(task), "the server could not take it");
  desktop.servers.end_claims(&Programs::processes()).await;

  assert_eq!(server.run_reports(), 2, "the next sync tried again");
  assert!(!server.claimed_now(task), "and the claim was completed");
}

#[tokio::test(flavor = "multi_thread")]
async fn live_updates_claim_ends_a_burst_of_run_changes_makes_no_more_rounds_than_the_minute() {
  let server = TeamServer::start(&[(ALICE, "Alice Moreau")], &[]);
  let (desktop, _task) = merged(&server).await;
  // Every report is refused for a moment, so each round of claim ends would send one.
  server.refuse_run_reports(1_000);
  let (runs, changes) = watch::channel(0);
  let work = tokio::spawn(orchester_workspace::sharing::project::work::run(
    desktop.servers.clone(),
    changes,
    Programs::processes(),
  ));
  let start = std::time::Instant::now();
  while server.run_reports() == 0 {
    assert!(start.elapsed() < Duration::from_secs(10), "the minute's first round");
    tokio::time::sleep(Duration::from_millis(20)).await;
  }

  for _ in 0..15 {
    runs.send_modify(|count| *count += 1);
    tokio::time::sleep(Duration::from_millis(150)).await;
  }
  tokio::time::sleep(Duration::from_secs(3)).await;

  assert_eq!(server.run_reports(), 1, "run changes ended no claims");
  work.abort();
}

#[tokio::test(flavor = "multi_thread")]
async fn live_updates_claim_ends_a_run_with_team_inputs_and_no_team_record_gets_no_claim_end() {
  let server = TeamServer::start(&[(ALICE, "Alice Moreau")], &[]);
  let (desktop, task) = merged(&server).await;
  let run = latest_run(&desktop, task);
  let database =
    rusqlite::Connection::open(desktop.root().join(".orchester/orchester.db")).expect("open the project database");
  // The inputs' content is not read: only that the run was created with the team's.
  database
    .execute(
      "INSERT INTO run_bundles (run_id, origin, operation_id, sha256, body) VALUES (?1, 'team', ?2, ?3, '{}')",
      [run.to_string(), OperationId::generate().to_string(), "0".repeat(64)],
    )
    .expect("team inputs");
  drop(database);

  desktop.servers.end_claims(&Programs::processes()).await;

  assert_eq!(server.run_reports(), 0, "no run progress report was sent");
  assert!(server.claimed_now(task), "the claim is not settled as a local one");
}
