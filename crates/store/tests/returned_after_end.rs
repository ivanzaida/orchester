//! The work of an ended run returned to a step (the project task ORC-44 · Return a finished run to an earlier step):
//! the attempt is stored and read back with its note and the conflicts it resolves; a return of a merged run is
//! refused. A merge that waits in a run's worktree for a person (*Resolve in editor*) begins only for a run that
//! succeeded and was not merged, and while it is recorded the run neither continues nor is merged.

mod support;

use orchester_core::{
  integration::{MergeKind, RunMerge},
  revision::Revision,
  run::{Answerer, AttemptReason, BranchName, MergeConflicts, ReviewNote, Run, RunStatus},
  store::{MergeStore, RunStore},
};
use orchester_store::{ProjectDb, ResolutionStart, RunStoreError, StoreError};

use self::support::{at, copied_at_version, started_run};

fn note() -> ReviewNote {
  ReviewNote::new("Merge main into this branch and resolve the conflicts listed below.").expect("a note")
}

fn conflicts() -> MergeConflicts {
  MergeConflicts {
    base: BranchName::new("main").expect("branch"),
    files: vec!["src/a.rs".to_owned(), "src/b.rs".to_owned()],
  }
}

fn merge_of(run: &Run) -> RunMerge {
  RunMerge {
    run: run.id(),
    into: BranchName::new("main").expect("branch"),
    revision: Revision::new(&"c".repeat(40)).expect("revision"),
    kind: MergeKind::FastForward,
    merged_at: at(15),
  }
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
fn returned_after_end_is_stored_with_its_note_and_conflicts() {
  let (repo, mut db, mut run) = succeeded();
  let step = run.ended_return_targets().remove(0);
  let events = run
    .return_ended(&step, (&note(), Some(&conflicts())), Answerer::Person, at(20))
    .expect("returned");
  db.append_events(&run, &events).expect("append");
  let reopened = ProjectDb::open(repo.path()).expect("open");
  let stored = reopened.run(run.id()).expect("read").expect("the run");
  assert_eq!(stored, run);
  assert_eq!(
    stored.current_attempt().expect("attempt").reason(),
    &AttemptReason::ReturnedAfterEnd {
      return_to: step,
      note: note(),
      conflicts: Some(conflicts()),
    }
  );
}

#[test]
fn returned_after_end_is_refused_for_a_merged_run() {
  let (_repo, mut db, mut run) = succeeded();
  db.save_run_merge(&merge_of(&run)).expect("merged");
  let before = db.run(run.id()).expect("read").expect("run");
  let step = run.ended_return_targets().remove(0);
  let events = run
    .return_ended(&step, (&note(), None), Answerer::Person, at(20))
    .expect("the run allows it");
  assert!(matches!(
    db.append_events(&run, &events),
    Err(RunStoreError::Merged(id)) if id == run.id()
  ));
  assert_eq!(db.run(run.id()).expect("read").expect("run"), before);
}

#[test]
fn returned_after_end_a_resolution_holds_the_run_back_and_its_merge() {
  let (_repo, mut db, mut run) = succeeded();
  assert_eq!(
    db.begin_resolution(run.id(), at(20)).expect("begin"),
    ResolutionStart::Begun
  );
  assert_eq!(db.resolution(run.id()).expect("read"), Some(at(20)));
  let step = run.ended_return_targets().remove(0);
  let mut returned = run.clone();
  let events = returned
    .return_ended(&step, (&note(), None), Answerer::Person, at(21))
    .expect("the run allows it");
  assert!(matches!(
    db.append_events(&returned, &events),
    Err(RunStoreError::Resolving(id)) if id == run.id()
  ));
  assert!(matches!(
    db.save_run_merge(&merge_of(&run)),
    Err(StoreError::NotMergeable(id)) if id == run.id()
  ));
  db.end_resolution(run.id()).expect("end");
  assert_eq!(db.resolution(run.id()).expect("read"), None);
  let events = run
    .return_ended(&step, (&note(), None), Answerer::Person, at(22))
    .expect("returned");
  db.append_events(&run, &events)
    .expect("the run continues once the merge in its worktree ended");
  // A run that works again neither begins a resolution nor is merged.
  assert_eq!(
    db.begin_resolution(run.id(), at(23)).expect("begin"),
    ResolutionStart::NotEnded
  );
  assert!(matches!(
    db.save_run_merge(&merge_of(&run)),
    Err(StoreError::NotMergeable(_))
  ));
}

#[test]
fn returned_after_end_a_removed_worktree_ends_its_resolution_and_an_older_one_ends_after_a_read() {
  use orchester_core::{integration::RemovalStart, store::WorktreeStore};
  let (_repo, mut db, run) = succeeded();
  db.begin_resolution(run.id(), at(20)).expect("begin");
  assert!(
    !db.end_resolution_before(run.id(), at(20)).expect("end"),
    "begun at the read: it stands"
  );
  assert!(
    db.end_resolution_before(run.id(), at(21)).expect("end"),
    "older than the read"
  );
  db.begin_resolution(run.id(), at(22)).expect("begin");
  assert_eq!(
    db.begin_worktree_removal(run.id(), at(23)).expect("remove"),
    RemovalStart::Recorded
  );
  assert_eq!(
    db.resolution(run.id()).expect("read"),
    None,
    "the merge in the worktree went with it"
  );
}

#[test]
fn returned_after_end_a_reopened_run_that_ended_otherwise_is_not_merged() {
  let (_repo, mut db, mut run) = succeeded();
  let step = run.ended_return_targets().remove(0);
  let events = run
    .return_ended(&step, (&note(), None), Answerer::Person, at(20))
    .expect("returned");
  db.append_events(&run, &events).expect("reopened");
  let events = run.cancel(at(21)).expect("cancelled");
  db.append_events(&run, &events).expect("cancelled");
  assert!(matches!(
    db.save_run_merge(&merge_of(&run)),
    Err(StoreError::NotMergeable(_))
  ));
}

#[test]
fn returned_after_end_a_merged_run_begins_no_resolution() {
  let (_repo, mut db, run) = succeeded();
  db.save_run_merge(&merge_of(&run)).expect("merged");
  assert_eq!(
    db.begin_resolution(run.id(), at(20)).expect("begin"),
    ResolutionStart::Merged
  );
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
  // Before both migrations of this change: the attempts table is rebuilt, the resolutions table is new.
  let old = copied_at_version(&current, latest - 2);
  let upgraded = ProjectDb::open(old.path()).expect("migrate");
  assert_eq!(upgraded.run(run.id()).expect("read").expect("the run"), run);
}
