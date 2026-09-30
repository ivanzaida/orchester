//! When indexes are built, reused and removed: one build per revision also when two runs ask at
//! once, removal with the last worktree that used a revision, and nothing left of a failed build.

mod support;

use std::{fs, sync::Barrier, thread};

use orchester_core::revision::Revision;
use orchester_index::{IndexError, Outcome};
use support::Repo;

/// The names in the index folder that are indexes, unfinished builds or run records.
fn index_folder(repo: &Repo) -> Vec<String> {
  let mut names: Vec<String> = fs::read_dir(repo.index_root())
    .expect("read the index folder")
    .map(|entry| entry.expect("entry").file_name().to_string_lossy().into_owned())
    .filter(|name| name != ".lock")
    .collect();
  names.sort();
  names
}

#[test]
fn an_existing_index_is_reused_by_the_next_run() {
  let repo = Repo::new();
  let head = repo.head();
  let first = repo.ensure(&head);
  assert!(matches!(first.outcome, Outcome::Built(_)));
  let second = repo.ensure(&head);
  assert!(matches!(second.outcome, Outcome::Reused));
  assert_eq!(first.path, second.path);
  assert_eq!(first.path, repo.index_root().join(head.as_str()));
}

#[test]
fn two_runs_asking_for_one_revision_at_once_build_it_once() {
  let repo = Repo::new();
  let head = repo.head();
  let runs = [repo.run_with_worktree(), repo.run_with_worktree()];
  let start = Barrier::new(runs.len());
  let outcomes: Vec<Outcome> = thread::scope(|scope| {
    let workers: Vec<_> = runs
      .iter()
      .map(|run| {
        let (repo, head, start) = (&repo, &head, &start);
        scope.spawn(move || {
          start.wait();
          repo.indexes().ensure(head, *run).expect("index").outcome
        })
      })
      .collect();
    workers
      .into_iter()
      .map(|worker| worker.join().expect("worker"))
      .collect()
  });
  let built = outcomes
    .iter()
    .filter(|outcome| matches!(outcome, Outcome::Built(_)))
    .count();
  assert_eq!(built, 1, "{outcomes:?}");
  assert_eq!(index_folder(&repo), [".users".to_owned(), head.to_string()]);
}

#[test]
fn an_index_is_removed_with_the_last_worktree_that_used_it() {
  let repo = Repo::new();
  let head = repo.head();
  let indexes = repo.indexes();
  let (first, second) = (repo.run_with_worktree(), repo.run_with_worktree());
  indexes.ensure(&head, first).expect("index");
  indexes.ensure(&head, second).expect("index");

  fs::remove_dir_all(repo.worktree(first)).expect("remove the first worktree");
  indexes.prune().expect("prune");
  assert!(
    repo.index_root().join(head.as_str()).is_dir(),
    "the second run still uses it"
  );

  fs::remove_dir_all(repo.worktree(second)).expect("remove the second worktree");
  indexes.prune().expect("prune");
  assert_eq!(index_folder(&repo), [".users".to_owned()]);
  assert_eq!(fs::read_dir(repo.index_root().join(".users")).expect("read").count(), 0);
}

#[test]
fn building_another_revision_removes_indexes_no_run_uses() {
  let repo = Repo::new();
  let first = repo.head();
  let run = repo.run_with_worktree();
  repo.indexes().ensure(&first, run).expect("index");
  fs::remove_dir_all(repo.worktree(run)).expect("remove the worktree");
  repo.write("README.md", "# Changed\n");
  repo.git(&["commit", "--quiet", "--all", "-m", "change"]);
  let second = repo.head();

  let ensured = repo.ensure(&second);

  assert!(
    matches!(&ensured.outcome, Outcome::Built(report) if report.read == 1),
    "built from the index of the first revision before removing it: {:?}",
    ensured.outcome
  );
  assert_eq!(index_folder(&repo), [".users".to_owned(), second.to_string()]);
}

#[test]
fn a_failed_build_leaves_nothing_behind() {
  let repo = Repo::new();
  let unknown = Revision::new(&"0".repeat(Revision::LEN)).expect("a revision");
  let run = repo.run_with_worktree();

  let error = repo.indexes().ensure(&unknown, run).expect_err("no such commit");

  assert!(matches!(error, IndexError::Git(_)), "{error}");
  assert_eq!(index_folder(&repo), [".users".to_owned()]);
}

#[test]
fn pruning_a_repository_without_indexes_does_nothing() {
  let repo = Repo::new();
  repo.indexes().prune().expect("prune");
  assert!(!repo.index_root().exists());
}
