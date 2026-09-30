//! When indexes are built, reused and removed: one build per revision also when two runs ask at
//! once, removal with the last worktree that used a revision, and nothing left of a failed build.

mod support;

use std::{
  fs,
  sync::{Barrier, mpsc},
  thread,
  time::Duration,
};

use orchester_core::revision::Revision;
use orchester_index::{BuildMode, BuildPhase, BuildProgress, IndexError, IndexState, Outcome};
use support::Repo;

/// The names in the index folder that are indexes, unfinished builds or run records.
fn index_folder(repo: &Repo) -> Vec<String> {
  let mut names: Vec<String> = fs::read_dir(repo.index_root())
    .expect("read the index folder")
    .map(|entry| entry.expect("entry").file_name().to_string_lossy().into_owned())
    .filter(|name| ![".locks", ".progress", ".failures"].contains(&name.as_str()))
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
fn a_failed_build_leaves_no_index_but_its_reason_until_its_run_is_gone() {
  let repo = Repo::new();
  let unknown = Revision::new(&"0".repeat(Revision::LEN)).expect("a revision");
  let run = repo.run_with_worktree();

  let error = repo.indexes().ensure(&unknown, run).expect_err("no such commit");

  assert!(matches!(error, IndexError::Git(_)), "{error}");
  assert_eq!(index_folder(&repo), [".users".to_owned()]);
  let listed = repo.indexes().list().expect("list");
  assert_eq!(listed.len(), 1);
  assert_eq!(listed[0].revision, unknown);
  assert_eq!(listed[0].runs, [run]);
  assert!(
    matches!(&listed[0].state, IndexState::Failed { reason } if *reason == error.to_string()),
    "{:?}",
    listed[0].state
  );

  fs::remove_dir_all(repo.worktree(run)).expect("remove the worktree");
  repo.indexes().prune().expect("prune");
  assert_eq!(repo.indexes().list().expect("list"), []);
}

#[test]
fn pruning_a_repository_without_indexes_does_nothing() {
  let repo = Repo::new();
  repo.indexes().prune().expect("prune");
  assert!(!repo.index_root().exists());
}

/// Holds the lock of `revision`'s index the way a build in another process does.
fn hold_lock(repo: &Repo, revision: &Revision) -> fs::File {
  let file = lock_file(repo, revision);
  file.lock().expect("lock");
  file
}

/// Holds the shared lock of `revision`'s index the way another revision's build reading it as its
/// base does.
fn hold_shared_lock(repo: &Repo, revision: &Revision) -> fs::File {
  let file = lock_file(repo, revision);
  file.lock_shared().expect("lock");
  file
}

fn lock_file(repo: &Repo, revision: &Revision) -> fs::File {
  let locks = repo.index_root().join(".locks");
  fs::create_dir_all(&locks).expect("create the locks folder");
  fs::File::create(locks.join(revision.as_str())).expect("open the lock")
}

#[test]
fn reusing_an_index_does_not_wait_for_a_build_reading_it_as_its_base() {
  let repo = Repo::new();
  let first = repo.head();
  repo.ensure(&first);
  repo.write(
    "README.md",
    "# Changed
",
  );
  repo.git(&["commit", "--quiet", "--all", "-m", "change"]);
  let second = repo.head();

  let _reading = hold_shared_lock(&repo, &first);
  let (done, finished) = mpsc::channel();
  let indexes = repo.indexes();
  let (run, reused) = (repo.run_with_worktree(), first);
  thread::spawn(move || {
    let outcome = indexes.ensure(&reused, run).map(|ensured| ensured.outcome);
    // The test may have given up waiting and ended.
    let _ = done.send(outcome);
  });
  let outcome = finished
    .recv_timeout(Duration::from_secs(30))
    .expect("reuse does not wait for the reader")
    .expect("the index is reused");
  assert!(matches!(outcome, Outcome::Reused));
  // Another build may read it as its base at the same time.
  let built = repo.ensure(&second);
  assert!(matches!(
    built.outcome,
    Outcome::Built(ref report) if matches!(report.mode, BuildMode::Incremental { .. })
  ));
}

#[test]
fn pruning_skips_a_revision_whose_index_is_busy_instead_of_waiting() {
  let repo = Repo::new();
  let head = repo.head();
  let run = repo.run_with_worktree();
  repo.indexes().ensure(&head, run).expect("index");
  fs::remove_dir_all(repo.worktree(run)).expect("remove the worktree");

  let held = hold_lock(&repo, &head);
  repo.indexes().prune().expect("prune");
  assert!(repo.index_root().join(head.as_str()).is_dir(), "a busy index is left");

  drop(held);
  repo.indexes().prune().expect("prune");
  assert!(!repo.index_root().join(head.as_str()).exists());
}

#[test]
fn a_build_of_one_revision_does_not_wait_for_another_revision() {
  let repo = Repo::new();
  let first = repo.head();
  repo.write("README.md", "# Changed\n");
  repo.git(&["commit", "--quiet", "--all", "-m", "change"]);
  let second = repo.head();

  let _held = hold_lock(&repo, &first);
  let ensured = repo.ensure(&second);

  assert!(matches!(ensured.outcome, Outcome::Built(_)));
}

#[test]
fn a_published_index_is_read_only_and_still_removed_with_its_worktree() {
  let repo = Repo::new();
  let head = repo.head();
  let run = repo.run_with_worktree();
  let index = repo.indexes().ensure(&head, run).expect("index").path;

  for name in [
    "tree.txt",
    "symbols.tsv",
    "manifest.json",
    "outline/crates/app/src/lib.rs.md",
  ] {
    let file = index.join(name);
    assert!(
      fs::metadata(&file).expect("metadata").permissions().readonly(),
      "{name}"
    );
    assert!(fs::write(&file, "changed").is_err(), "{name} cannot be written");
  }
  fs::remove_dir_all(repo.worktree(run)).expect("remove the worktree");
  repo.indexes().prune().expect("prune");
  assert!(!index.exists());
}

fn folder_size(folder: &std::path::Path) -> u64 {
  fs::read_dir(folder)
    .expect("read")
    .map(|entry| {
      let entry = entry.expect("entry");
      let metadata = entry.metadata().expect("metadata");
      if metadata.is_dir() {
        folder_size(&entry.path())
      } else {
        metadata.len()
      }
    })
    .sum()
}

#[test]
fn the_list_is_read_from_records_while_runs_read_the_index() {
  let repo = Repo::new();
  let head = repo.head();
  repo.ensure(&head);
  // A run reusing the index shares its lock; a stale progress record is no build.
  let _reading = hold_shared_lock(&repo, &head);
  let progress = repo.index_root().join(".progress");
  fs::create_dir_all(&progress).expect("progress folder");
  fs::write(
    progress.join(head.as_str()),
    r#"{"started_at":0,"phase":{"kind":"writing"}}"#,
  )
  .expect("a stale record");

  let listed = repo.indexes().list().expect("list");
  assert!(matches!(listed[0].state, IndexState::Ready(_)), "{:?}", listed[0].state);
}

#[test]
fn the_list_shows_ready_indexes_with_their_runs_and_rebuilds_replace_them() {
  let repo = Repo::new();
  let head = repo.head();
  let run = repo.run_with_worktree();
  let first = repo.indexes().ensure(&head, run).expect("index");

  let listed = repo.indexes().list().expect("list");
  assert_eq!(listed.len(), 1);
  assert_eq!(listed[0].runs, [run]);
  let IndexState::Ready(summary) = &listed[0].state else {
    panic!("ready: {:?}", listed[0].state);
  };
  assert_eq!((summary.files, summary.symbols), (15, 42));
  // The size of the index's files, recorded when it was built, not measured by the list.
  let manifest = fs::metadata(first.path.join("manifest.json")).expect("manifest").len();
  assert_eq!(summary.bytes, folder_size(&first.path) - manifest);

  let report = repo.indexes().rebuild(&head).expect("rebuilt");
  assert_eq!(report.mode, orchester_index::BuildMode::Full);
  let IndexState::Ready(rebuilt) = &repo.indexes().list().expect("list")[0].state else {
    panic!("ready after the rebuild");
  };
  assert!(rebuilt.built_at >= summary.built_at);
  assert_eq!(repo.indexes().existing(&head), Some(first.path));
}

#[test]
fn the_list_shows_a_running_build_with_its_phase() {
  let repo = Repo::new();
  let head = repo.head();
  let held = hold_lock(&repo, &head);
  let progress = repo.index_root().join(".progress");
  fs::create_dir_all(&progress).expect("create");
  let building = BuildProgress {
    started_at: orchester_core::time::Timestamp::from_unix_millis(1_000),
    phase: BuildPhase::Indexing { files: 15 },
  };
  fs::write(
    progress.join(head.as_str()),
    serde_json::to_string(&building).expect("json"),
  )
  .expect("write");

  let listed = repo.indexes().list().expect("list");
  assert_eq!(listed[0].state, IndexState::Building(building));

  drop(held);
  assert_eq!(
    repo.indexes().list().expect("list"),
    [],
    "a record without a build shows nothing"
  );
}
