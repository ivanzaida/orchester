//! *Resolve in editor* on a run's worktree (`tasks::resolve`): the merge is recorded as the run's resolution before
//! git begins it, so the run neither continues nor is merged while it waits; *Abort* and *Continue* end it, and a
//! merge a person ended themselves ends it when the worktree is read again.

use std::{fs, path::Path};

use orchester_core::integration::resolve::Started;
use orchester_exec::programs::Programs;
use orchester_store::ProjectDb;
use orchester_workspace::tasks::resolve;

use super::support::{Fixture, git};

/// A succeeded run whose `task.txt` main changed too since it started.
fn conflicting() -> Fixture {
  let fixture = Fixture::recorded(false, "resolve-", "task", Some("delivered task\n"));
  fs::write(fixture.root().join("task.txt"), "main's task\n").expect("write");
  git(fixture.root(), &["commit", "-am", "main's change"]);
  fixture
}

fn programs() -> Programs {
  Programs::processes()
}

fn resolution(root: &Path, fixture: &Fixture) -> Option<orchester_core::time::Timestamp> {
  ProjectDb::open(root)
    .expect("db")
    .resolution(fixture.run)
    .expect("read")
}

#[test]
fn local_integration_resolve_records_the_merge_and_holds_the_run_back_until_it_ends() {
  let fixture = conflicting();
  let root = fixture.root();
  let started = resolve::start(&programs(), root, fixture.run).expect("merged into the worktree");
  assert_eq!(started, Started::Conflicts(vec!["task.txt".to_owned()]));
  assert!(resolution(root, &fixture).is_some());
  let worktree = resolve::read(&programs(), root, (fixture.run, true)).expect("read");
  assert!(worktree.present);
  assert!(worktree.merge.is_some());

  // The store refuses the run's continuations meanwhile (`orchester-store` `returned_after_end`).
  resolve::abort(&programs(), root, fixture.run).expect("aborted");
  assert_eq!(resolution(root, &fixture), None);
  assert_eq!(git(&fixture.task_root, &["status", "--porcelain"]), "");
}

#[test]
fn local_integration_resolve_a_merge_ended_outside_orchester_ends_its_resolution() {
  let fixture = conflicting();
  let root = fixture.root();
  resolve::start(&programs(), root, fixture.run).expect("merged into the worktree");
  git(&fixture.task_root, &["merge", "--abort"]);
  // While a resolving step works, a missing merge may be one it is about to begin: nothing ends then.
  resolve::read(&programs(), root, (fixture.run, false)).expect("read");
  assert!(resolution(root, &fixture).is_some());
  let worktree = resolve::read(&programs(), root, (fixture.run, true)).expect("read");
  assert!(worktree.merge.is_none());
  assert_eq!(resolution(root, &fixture), None);
}

#[test]
fn local_integration_resolve_continue_commits_and_ends_the_resolution() {
  let fixture = conflicting();
  let root = fixture.root();
  resolve::start(&programs(), root, fixture.run).expect("merged into the worktree");
  assert!(
    resolve::finish(&programs(), root, fixture.run).is_err(),
    "markers are left"
  );
  fs::write(fixture.task_root.join("task.txt"), "main's task\ndelivered task\n").expect("resolved");
  resolve::finish(&programs(), root, fixture.run).expect("committed");
  assert_eq!(resolution(root, &fixture), None);
  let main = git(root, &["rev-parse", "main"]);
  git(&fixture.task_root, &["merge-base", "--is-ancestor", &main, "HEAD"]);
}
