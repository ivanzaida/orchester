//! A run whose stored record cannot be read back is listed as unreadable beside the runs that can
//! be read; reading it by its id returns the typed error. Nothing writes over its record.

mod support;

use std::path::Path;

use orchester_core::{
  id::{IdempotencyKey, RunId, TaskId},
  revision::Revision,
  run::{BranchName, InterruptCause, ListedRun, NewRun, Run, UnreadableRun},
  store::RunStore,
};
use orchester_store::{ProjectDb, RunStoreError, StoreError};
use rusqlite::Connection;
use support::{at, budget, started_run};
use tempfile::TempDir;

/// A task with three started runs, numbered 1 to 3.
struct Runs {
  repo: TempDir,
  db: ProjectDb,
  task: TaskId,
  runs: Vec<Run>,
}

impl Runs {
  fn new() -> Self {
    let (repo, mut db, first, _) = started_run();
    let mut runs = vec![first.clone()];
    for number in 2..=3 {
      runs.push(start(&mut db, &first, number, IdempotencyKey::generate()));
    }
    Self {
      repo,
      db,
      task: first.task(),
      runs,
    }
  }

  fn tamper(&self, run: RunId, assignments: &str) {
    tamper(self.repo.path(), run, assignments);
  }
}

fn raw(repo: &Path) -> Connection {
  Connection::open(repo.join(".orchester/orchester.db")).expect("open raw database")
}

/// Sets `assignments` on the row of `run`, as an edit or damage outside Orchester would.
fn tamper(repo: &Path, run: RunId, assignments: &str) {
  raw(repo)
    .execute(
      &format!("UPDATE runs SET {assignments} WHERE id = ?1"),
      [run.to_string()],
    )
    .expect("tamper");
}

/// The row of `run` as stored: status, last sequence and snapshot.
fn stored(repo: &Path, run: RunId) -> (String, i64, String) {
  raw(repo)
    .query_row(
      "SELECT status, last_sequence, snapshot FROM runs WHERE id = ?1",
      [run.to_string()],
      |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    )
    .expect("stored row")
}

/// Creates and starts run `number` of the task of `first`, with the same configuration.
fn start(db: &mut ProjectDb, first: &Run, number: u32, key: IdempotencyKey) -> Run {
  let new = NewRun {
    task: first.task(),
    snapshot: first.snapshot().clone(),
    branch: BranchName::new(&format!("orchester/orc-1-run-{number}")).expect("branch"),
    base: Revision::new(&"b".repeat(40)).expect("revision"),
    base_branch: None,
    budget: budget(1_000_000),
    max_duration: first.max_duration(),
  };
  let mut run = db.create_run(key, new, at(20)).expect("run");
  let events = run.start(at(21)).expect("start");
  db.append_events(&run, &events).expect("append");
  run
}

/// The run of `listed`, which must be readable.
fn readable(listed: &ListedRun<RunStoreError>) -> &Run {
  listed.as_ref().expect("readable run")
}

/// The unreadable run of `listed`.
fn unreadable(listed: &ListedRun<RunStoreError>) -> &UnreadableRun<RunStoreError> {
  listed.as_ref().expect_err("unreadable run")
}

#[test]
fn unreadable_run_breaking_its_rules_is_listed_beside_the_good_runs() {
  let fixture = Runs::new();
  let bad = fixture.runs[1].id();
  // A started run has an attempt; a pending one never has.
  fixture.tamper(bad, "status = 'pending'");

  let listed = fixture.db.runs_of_task(fixture.task).expect("the listing reads");
  assert_eq!(listed.len(), 3);
  assert_eq!(readable(&listed[0]), &fixture.runs[0]);
  assert_eq!(readable(&listed[2]), &fixture.runs[2]);
  let entry = unreadable(&listed[1]);
  assert_eq!(
    (entry.id, entry.task, entry.number),
    (bad, fixture.task, Some(fixture.runs[1].number()))
  );
  assert!(matches!(
    &entry.error,
    RunStoreError::Unreadable {
      run,
      source: StoreError::Corrupt { what: "run", .. },
    } if *run == bad
  ));

  assert!(matches!(
    fixture.db.run(bad),
    Err(RunStoreError::Unreadable {
      run,
      source: StoreError::Corrupt { what: "run", .. },
    }) if run == bad
  ));
  assert_eq!(
    fixture.db.run(fixture.runs[2].id()).expect("read"),
    Some(fixture.runs[2].clone())
  );
}

#[test]
fn unreadable_run_with_undecodable_json_is_listed_among_the_running_runs() {
  let fixture = Runs::new();
  let bad = fixture.runs[0].id();
  fixture.tamper(bad, "snapshot = 'not json'");

  let running = fixture.db.running_runs().expect("the listing reads");
  assert_eq!(running.len(), 3);
  assert!(matches!(
    &unreadable(&running[0]).error,
    RunStoreError::Unreadable {
      source: StoreError::Corrupt {
        what: "run snapshot",
        ..
      },
      ..
    }
  ));
  let good: Vec<RunId> = running[1..].iter().map(|run| readable(run).id()).collect();
  assert_eq!(good, [fixture.runs[1].id(), fixture.runs[2].id()]);
  assert!(matches!(
    fixture.db.run(bad),
    Err(RunStoreError::Unreadable {
      source: StoreError::Corrupt {
        what: "run snapshot",
        ..
      },
      ..
    })
  ));
}

#[test]
fn unreadable_run_is_listed_among_the_interrupted_runs() {
  let mut fixture = Runs::new();
  for index in [0, 2] {
    let mut run = fixture.runs[index].clone();
    let events = run.interrupt(InterruptCause::RunnerLost, at(30)).expect("interrupt");
    fixture.db.append_events(&run, &events).expect("append");
    fixture.runs[index] = run;
  }
  let bad = fixture.runs[2].id();
  fixture.tamper(bad, "status_reason = 'not a cause'");

  let interrupted = fixture.db.interrupted_runs().expect("the listing reads");
  assert_eq!(interrupted.len(), 2);
  assert_eq!(readable(&interrupted[0]), &fixture.runs[0]);
  assert_eq!(unreadable(&interrupted[1]).id, bad);
}

#[test]
fn unreadable_run_is_never_created_again_or_written_over() {
  let (repo, mut db, first, _) = started_run();
  let key = IdempotencyKey::generate();
  let second = start(&mut db, &first, 2, key);
  tamper(repo.path(), second.id(), "snapshot = 'not json'");
  let before = stored(repo.path(), second.id());

  let new = NewRun {
    task: first.task(),
    snapshot: first.snapshot().clone(),
    branch: BranchName::new("orchester/orc-1-run-2").expect("branch"),
    base: Revision::new(&"b".repeat(40)).expect("revision"),
    base_branch: None,
    budget: budget(1_000_000),
    max_duration: first.max_duration(),
  };
  assert!(matches!(
    db.create_run(key, new, at(40)),
    Err(RunStoreError::Unreadable { run, .. }) if run == second.id()
  ));
  assert_eq!(db.runs_of_task(first.task()).expect("list").len(), 2);
  assert_eq!(stored(repo.path(), second.id()), before);
}

/// A row whose id is not a run id cannot be told apart from the others: the listing fails rather
/// than guess which run it is.
#[test]
fn unreadable_run_without_an_id_fails_the_listing() {
  let fixture = Runs::new();
  let raw = raw(fixture.repo.path());
  raw.execute_batch("PRAGMA foreign_keys = OFF").expect("pragma");
  raw
    .execute(
      "UPDATE runs SET id = 'not-an-id' WHERE id = ?1",
      [fixture.runs[1].id().to_string()],
    )
    .expect("tamper");
  assert!(matches!(
    fixture.db.runs_of_task(fixture.task),
    Err(RunStoreError::Store(StoreError::Corrupt { what: "run id", .. }))
  ));
}
