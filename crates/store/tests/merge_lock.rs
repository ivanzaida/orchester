//! Recording a merge keeps the project's write lock only for the merge itself (ORC-914): what the task took is read
//! before the write transaction and by an index of the run, not by a walk over every turn of the project. Run with
//! `cargo test -p orchester-store --test merge_lock merge_lock`.

mod support;

use std::time::{Duration, Instant};

use orchester_core::{
  integration::{MergeKind, RunMerge},
  revision::Revision,
  run::BranchName,
  store::MergeStore,
};
use orchester_store::StoreError;
use rusqlite::{Connection, params};

use self::support::{at, started_run};

/// Turns of other tasks' runs, enough for a scan to show in a plan.
const OTHER_TURNS: i64 = 5_000;

/// The statements that read a run's events by kind, as the store words them (`task_effort.rs`, `activity.rs`), with
/// the values they take.
const BY_RUN_AND_KIND: [(&str, &[&str]); 2] = [
  (
    "SELECT payload FROM agent_events WHERE kind = 'turn_completed' AND run_id IN (SELECT id FROM runs WHERE task_id = ?1)",
    &["task"],
  ),
  (
    "SELECT sequence, payload FROM agent_events WHERE run_id = ?1 AND step_execution_id = ?2 AND kind = 'message'      ORDER BY sequence DESC LIMIT 1",
    &["run", "step"],
  ),
];

fn merge_of(run: orchester_core::id::RunId) -> RunMerge {
  RunMerge {
    run,
    into: BranchName::new("main").expect("branch"),
    revision: Revision::new(&"c".repeat(40)).expect("revision"),
    kind: MergeKind::MergeCommit,
    merged_at: at(30),
  }
}

/// A second connection to the project's database, which does not check the references of the rows it plants.
fn raw_connection(repo: &std::path::Path) -> Connection {
  let raw = Connection::open(repo.join(".orchester").join("orchester.db")).expect("open");
  raw.pragma_update(None, "foreign_keys", false).expect("foreign keys");
  raw
}

fn insert_turn(connection: &Connection, run: &str, sequence: i64, payload: &str) {
  connection
    .execute(
      "INSERT INTO agent_events (run_id, sequence, step_execution_id, invocation_id, at, kind, payload) \
       VALUES (?1, ?2, 'step', 'invocation', ?2, 'turn_completed', ?3)",
      params![run, sequence, payload],
    )
    .expect("insert");
}

#[test]
fn merge_lock_reads_a_runs_events_by_kind_through_the_run_index() {
  let (repo, _db, _run, _) = started_run();
  let raw = raw_connection(repo.path());
  raw.execute_batch("BEGIN").expect("begin");
  for sequence in 1..=OTHER_TURNS {
    insert_turn(&raw, "another-run", sequence, "{}");
  }
  raw.execute_batch("COMMIT").expect("commit");

  for (statement, values) in BY_RUN_AND_KIND {
    let mut plan = raw.prepare(&format!("EXPLAIN QUERY PLAN {statement}")).expect("plan");
    let details = plan
      .query_map(rusqlite::params_from_iter(values), |row| row.get::<_, String>(3))
      .expect("rows")
      .collect::<Result<Vec<_>, _>>()
      .expect("details");
    let events: Vec<&String> = details
      .iter()
      .filter(|detail| detail.contains("agent_events"))
      .collect();
    assert!(!events.is_empty(), "{statement}: {details:?}");
    assert!(
      events
        .iter()
        .all(|detail| detail.contains("run_id=") && !detail.contains("agent_events_by_kind")),
      "{statement} is searched by run, not by kind: {details:?}"
    );
  }
}

#[test]
fn merge_lock_measures_what_the_task_took_before_it_takes_the_write_lock() {
  let (repo, mut db, run, _) = started_run();
  let raw = raw_connection(repo.path());
  // A transcript the effort cannot read stops the merge at the measurement.
  insert_turn(&raw, &run.id().to_string(), 1_000_000, "not json");
  raw.execute_batch("BEGIN IMMEDIATE").expect("hold the write lock");

  let started = Instant::now();
  let error = db.save_run_merge(&merge_of(run.id())).expect_err("unreadable turn");
  assert!(
    matches!(error, StoreError::Corrupt { .. }),
    "stopped at the measurement, not at the lock: {error:?}"
  );
  assert!(
    started.elapsed() < Duration::from_secs(5),
    "the measurement waited for the write lock: {:?}",
    started.elapsed()
  );
  raw.execute_batch("ROLLBACK").expect("let go");
}
