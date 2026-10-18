//! The work of an ended run returned to a step (ORC-44): the attempt is stored and read back with its note, and a
//! return of a run whose work was merged is refused without recording anything.

mod support;

use orchester_core::{
  integration::{MergeKind, RunMerge},
  revision::Revision,
  run::{AttemptReason, BranchName, ReviewNote, Run, RunStatus},
  store::{MergeStore, RunStore},
};
use orchester_store::{ProjectDb, RunStoreError};

use self::support::{at, copied_at_version, started_run};

fn note() -> ReviewNote {
  ReviewNote::new("Merge main into this branch and resolve the conflicts.").expect("a note")
}

/// [`started_run`] whose one step passed: the run succeeded.
fn succeeded() -> (tempfile::TempDir, ProjectDb, Run) {
  let (repo, mut db, mut run, execution) = started_run();
  let mut events = run.start_step(execution, at(10)).expect("start step");
  events.extend(run.complete_step(execution, at(11)).expect("pass step"));
  db.append_events(&run, &events).expect("append");
  assert_eq!(run.status(), &RunStatus::Succeeded);
  (repo, db, run)
}

#[test]
fn returned_after_end_is_stored_with_its_note() {
  let (repo, mut db, mut run) = succeeded();
  let step = run.ended_return_targets().remove(0);
  let events = run.return_ended(&step, &note(), at(20)).expect("returned");
  db.append_events(&run, &events).expect("append");
  let reopened = ProjectDb::open(repo.path()).expect("open");
  let stored = reopened.run(run.id()).expect("read").expect("the run");
  assert_eq!(stored, run);
  assert_eq!(
    stored.current_attempt().expect("attempt").reason(),
    &AttemptReason::ReturnedAfterEnd {
      return_to: step,
      note: note()
    }
  );
}

#[test]
fn returned_after_end_is_refused_for_a_merged_run() {
  let (_repo, mut db, mut run) = succeeded();
  db.save_run_merge(&RunMerge {
    run: run.id(),
    into: BranchName::new("main").expect("branch"),
    revision: Revision::new(&"c".repeat(40)).expect("revision"),
    kind: MergeKind::FastForward,
    merged_at: at(15),
  })
  .expect("merged");
  let before = db.run(run.id()).expect("read").expect("run");
  let step = run.ended_return_targets().remove(0);
  let events = run.return_ended(&step, &note(), at(20)).expect("the run allows it");
  assert!(matches!(
    db.append_events(&run, &events),
    Err(RunStoreError::Merged(id)) if id == run.id()
  ));
  assert_eq!(db.run(run.id()).expect("read").expect("run"), before);
}

#[test]
fn returned_after_end_migration_keeps_the_attempts() {
  let (repo, db, run) = succeeded();
  drop(db);
  let current = repo.path().join(".orchester/orchester.db");
  let latest: u32 = rusqlite::Connection::open(&current)
    .expect("open")
    .pragma_query_value(None, "user_version", |row| row.get(0))
    .expect("version");
  let old = copied_at_version(&current, latest - 1);
  let upgraded = ProjectDb::open(old.path()).expect("migrate");
  assert_eq!(upgraded.run(run.id()).expect("read").expect("the run"), run);
}
