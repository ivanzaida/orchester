//! What integrating runs stores: the branch a run started on, a run's merge, once, when a run's
//! worktree was removed, and the proven rebase of a run's branch.

mod support;

use orchester_core::{
  id::{IdempotencyKey, RunId},
  integration::{
    MergeKind, RemovalStart, RunMerge,
    base::RunBaseSource,
    local::{RebaseEvidence, RebaseStore, RebasedCheckpoint, RunRebase},
  },
  revision::Revision,
  run::{BranchName, InterruptCause, NewRun},
  store::{MergeStore, RunStore, WorktreeStore},
};
use orchester_store::{ProjectDb, RunStoreError, StoreError};

use self::support::{at, budget, copied_at_version, started_run};

fn branch(name: &str) -> BranchName {
  BranchName::new(name).expect("valid branch")
}

#[test]
fn integration_base_branch_is_kept_with_the_run() {
  let (repo, mut db, first, _) = started_run();
  let new = NewRun {
    task: first.task(),
    snapshot: first.snapshot().clone(),
    branch: branch("orchester/orc-1-run-2"),
    base: Revision::new(&"b".repeat(40)).expect("revision"),
    base_branch: Some(branch("main")),
    base_source: RunBaseSource::CheckedOut,
    budget: Some(budget(1_000_000)),
    max_duration: first.max_duration(),
  };
  let second = db.create_run(IdempotencyKey::generate(), new, at(20)).expect("run");
  let reopened = ProjectDb::open(repo.path()).expect("open");
  let runs: Vec<_> = reopened
    .runs_of_task(first.task())
    .expect("runs")
    .into_iter()
    .map(|run| run.expect("readable run"))
    .collect();
  assert_eq!(runs[0].base_branch(), None, "the run started on a detached HEAD");
  assert_eq!(runs[1].id(), second.id());
  assert_eq!(runs[1].base_branch(), Some(&branch("main")));
}

#[test]
fn integration_base_branch_counts_the_runs_that_keep_their_base() {
  let (_repo, mut db, mut run, _) = started_run();
  assert_eq!(db.runs_keeping_their_base().expect("count"), 1, "the running run");
  let events = run.cancel(at(40)).expect("cancel");
  db.append_events(&run, &events).expect("events");
  assert_eq!(
    db.runs_keeping_their_base().expect("count"),
    0,
    "a cancelled run delivers nothing"
  );
}

#[test]
fn integration_merge_is_recorded_once_per_run() {
  let (repo, mut db, run, _) = started_run();
  assert_eq!(db.run_merge(run.id()).expect("read"), None);
  let merge = RunMerge {
    run: run.id(),
    into: branch("main"),
    revision: Revision::new(&"c".repeat(40)).expect("revision"),
    kind: MergeKind::MergeCommit,
    merged_at: at(30),
  };
  db.save_run_merge(&merge).expect("save");
  let again = RunMerge {
    kind: MergeKind::FastForward,
    ..merge.clone()
  };
  assert!(matches!(db.save_run_merge(&again), Err(StoreError::AlreadyMerged(id)) if id == run.id()));
  let reopened = ProjectDb::open(repo.path()).expect("open");
  assert_eq!(reopened.run_merge(run.id()).expect("read"), Some(merge.clone()));
  let unknown = RunMerge {
    run: RunId::generate(),
    ..merge
  };
  assert!(
    db.save_run_merge(&unknown).is_err(),
    "a merge belongs to a recorded run"
  );
}

#[test]
fn integration_worktree_removal_is_recorded_once_and_ends_an_interrupted_run_for_good() {
  let (repo, mut db, mut run, _) = started_run();
  let events = run.interrupt(InterruptCause::QuitNow, at(20)).expect("interrupt");
  db.append_events(&run, &events).expect("append");
  assert_eq!(db.interrupted_runs().expect("listed").len(), 1);
  assert_eq!(db.worktree_removal(run.id()).expect("read"), None);
  db.save_worktree_removal(run.id(), at(30)).expect("save");
  db.save_worktree_removal(run.id(), at(40))
    .expect("a second removal changes nothing");
  let reopened = ProjectDb::open(repo.path()).expect("open");
  assert_eq!(reopened.worktree_removal(run.id()).expect("read"), Some(at(30)));
  assert_eq!(
    reopened.worktree_removals(run.task()).expect("read"),
    vec![(run.id(), at(30))]
  );
  assert!(
    reopened.interrupted_runs().expect("listed").is_empty(),
    "it can never continue, so recovery leaves it out"
  );
  assert!(
    db.save_worktree_removal(RunId::generate(), at(50)).is_err(),
    "a removal belongs to a recorded run"
  );
}

#[test]
fn integration_worktree_removal_and_continuation_never_both_go_ahead() {
  let (_repo, mut db, mut run, _) = started_run();
  // Running: a removal is refused and records nothing.
  assert_eq!(
    db.begin_worktree_removal(run.id(), at(20)).expect("read"),
    RemovalStart::LiveWork
  );
  assert_eq!(db.worktree_removal(run.id()).expect("read"), None);
  let events = run.interrupt(InterruptCause::QuitNow, at(21)).expect("interrupt");
  db.append_events(&run, &events).expect("append");
  assert_eq!(db.active_task_run(run.task()).expect("read"), Some(run.id()));

  // Interrupted: the removal is recorded first; a continuation made from the run as read before
  // is then refused and changes nothing.
  assert_eq!(
    db.begin_worktree_removal(run.id(), at(30)).expect("record"),
    RemovalStart::Recorded
  );
  let before = db.run(run.id()).expect("read").expect("run");
  let mut continued = run.clone();
  let events = continued.continue_run(at(31)).expect("continue");
  assert!(matches!(
    db.append_events(&continued, &events),
    Err(RunStoreError::WorktreeRemoved(id)) if id == run.id()
  ));
  assert_eq!(db.run(run.id()).expect("read").expect("run"), before);
  assert_eq!(
    db.active_task_run(run.task()).expect("read"),
    None,
    "it never continues, so Canceled stops nothing"
  );
  assert_eq!(
    db.begin_worktree_removal(run.id(), at(40)).expect("read"),
    RemovalStart::AlreadyRecorded
  );

  // Git removed nothing: the record is taken back and the run continues as before.
  db.undo_worktree_removal(run.id(), at(30)).expect("undo");
  assert_eq!(db.worktree_removal(run.id()).expect("read"), None);
  db.append_events(&continued, &events)
    .expect("the continuation is recorded");
  assert_eq!(
    db.begin_worktree_removal(run.id(), at(50)).expect("read"),
    RemovalStart::LiveWork
  );
}

/// The schema version before removals were recorded: task organization (39).
const BEFORE_REMOVALS: u32 = 39;

#[test]
fn integration_worktree_removals_migration_keeps_runs_of_schema_39() {
  let (repo, mut db, mut run, _) = started_run();
  let events = run.interrupt(InterruptCause::QuitNow, at(20)).expect("interrupt");
  db.append_events(&run, &events).expect("append");
  drop(db);
  let current = repo.path().join(".orchester").join("orchester.db");
  let old = copied_at_version(&current, BEFORE_REMOVALS);
  let mut db = ProjectDb::open(old.path()).expect("migrate existing project");
  assert_eq!(db.run(run.id()).expect("read").expect("run"), run);
  assert_eq!(
    db.worktree_removal(run.id()).expect("read"),
    None,
    "no removal was recorded before"
  );
  assert_eq!(db.interrupted_runs().expect("listed").len(), 1);
  assert_eq!(db.active_task_run(run.task()).expect("read"), Some(run.id()));
  assert_eq!(
    db.begin_worktree_removal(run.id(), at(30)).expect("record"),
    RemovalStart::Recorded
  );
  assert!(db.interrupted_runs().expect("listed").is_empty());
}

#[test]
fn integration_a_rebase_is_kept_per_run_and_a_newer_one_replaces_it() {
  let (repo, mut db, run, _) = started_run();
  assert_eq!(db.run_rebase(run.id()).expect("read"), None);
  let revision = |c: &str| Revision::new(&c.repeat(40)).expect("revision");
  let evidence = |onto: &str, checkpoint| RebaseEvidence {
    onto: revision(onto),
    newer_than_base: true,
    under_tip_and_target: true,
    own_work_below: false,
    checkpoint,
  };
  let first = RunRebase::proven(run.id(), evidence("d", None), at(40)).expect("proven");
  db.save_run_rebase(&first).expect("save");
  let rewritten = RebasedCheckpoint {
    replaces: revision("e"),
    tip: revision("f"),
  };
  let newer = RunRebase::proven(run.id(), evidence("a", Some(rewritten)), at(50)).expect("proven");
  db.save_run_rebase(&newer).expect("replace");
  let reopened = ProjectDb::open(repo.path()).expect("open");
  assert_eq!(reopened.run_rebase(run.id()).expect("read"), Some(newer));
  let own_work = RebaseEvidence {
    own_work_below: true,
    ..evidence("b", None)
  };
  assert_eq!(RunRebase::proven(run.id(), own_work, at(60)), None);
}
