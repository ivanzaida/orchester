//! A run that moved to a free branch before it started, one that failed as its worktree could not be created, and
//! the migration that let a failed run have no failed step: runs that failed before keep their step.

mod support;

use orchester_core::{
  run::{BranchName, GitMessage, Run, RunFailure, RunStatus, WorktreeFailure},
  store::RunStore,
};
use orchester_store::ProjectDb;
use rusqlite::Connection;

use self::support::{at, copied_at_version, pending_run, started_run};

/// The schema before the migration that let a failed run have no failed step (`worktree_not_created`, the 67th).
/// Migrations are append-only.
const BEFORE_NOT_STARTED: u32 = 66;

fn reloaded(db: &ProjectDb, run: &Run) -> Run {
  db.run(run.id()).expect("read").expect("run")
}

#[test]
fn a_chosen_branch_is_stored_as_the_runs_branch() {
  let (_repo, mut db, mut run) = pending_run();
  let free = BranchName::new("orchester/orc-1-run-1-2").expect("branch");

  let events = run.choose_branch(free.clone(), at(7)).expect("chosen");
  db.append_events(&run, &events).expect("stored");

  assert_eq!(reloaded(&db, &run).branch(), &free);
  let branches = db.run_branches().expect("run branches");
  assert_eq!(branches[0].branch, free, "what pull requests and merging read");
  let events = run.start(at(8)).expect("start");
  db.append_events(&run, &events).expect("stored start");
  assert_eq!(reloaded(&db, &run), run);
}

#[test]
fn a_run_that_could_not_start_keeps_its_reason_and_owns_no_branch() {
  let (repo, mut db, mut run) = pending_run();
  let reason = WorktreeFailure::Git(GitMessage::new("fatal: disk full"));

  let events = run.fail_to_start(reason.clone(), at(7)).expect("failed");
  db.append_events(&run, &events).expect("stored");

  let stored = reloaded(&db, &run);
  assert_eq!(stored, run);
  assert!(
    db.run_branches().expect("run branches").is_empty(),
    "no pull request is its"
  );
  assert_eq!(
    stored.status(),
    &RunStatus::Failed(RunFailure::WorktreeNotCreated(reason))
  );
  drop(db);
  let columns: (String, Option<String>, Option<String>) = Connection::open(repo.path().join(".orchester/orchester.db"))
    .expect("open")
    .query_row(
      "SELECT status_reason, failed_step, failure_reason FROM runs WHERE id = ?1",
      [run.id().to_string()],
      |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    )
    .expect("row");
  assert_eq!(
    columns,
    (
      "worktree_not_created".to_owned(),
      None,
      Some(r#"{"git":"fatal: disk full"}"#.to_owned())
    )
  );
}

#[test]
fn runs_that_failed_before_the_migration_keep_their_step() {
  let (repo, mut db, mut run, execution) = started_run();
  let events = run.start_step(execution, at(10)).expect("started step");
  db.append_events(&run, &events).expect("stored step start");
  let events = run
    .fail_step(execution, orchester_core::run::StepFailure::ReportedFailed, at(11))
    .expect("failed");
  db.append_events(&run, &events).expect("stored failure");
  assert!(matches!(run.status(), RunStatus::Failed(RunFailure::StepFailed(_))));
  drop(db);
  let current = repo.path().join(".orchester").join("orchester.db");

  let old = copied_at_version(&current, BEFORE_NOT_STARTED);

  let db = ProjectDb::open(old.path()).expect("migrated");
  assert_eq!(reloaded(&db, &run), run);
}
