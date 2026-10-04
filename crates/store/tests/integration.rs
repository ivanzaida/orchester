//! What integrating runs stores: the branch a run started on, a run's merge, once, and when a run's
//! worktree was removed.

mod support;

use orchester_core::{
  id::{IdempotencyKey, RunId},
  integration::{MergeKind, RunMerge},
  revision::Revision,
  run::{BranchName, InterruptCause, NewRun},
  store::{MergeStore, RunStore, WorktreeStore},
};
use orchester_store::{ProjectDb, StoreError};

use self::support::{at, budget, started_run};

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
    budget: budget(1_000_000),
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
