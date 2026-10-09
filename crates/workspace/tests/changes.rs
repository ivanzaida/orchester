//! The count of files the Changes tab lists, shown on its label and in the Conversation, the worktree
//! reads that follow a page only while it is there, and what a missing worktree means.

use std::{
  sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
  },
  time::Duration,
};

use orchester_core::{
  diff::{Changes, Diff},
  id::RunId,
  revision::Revision,
};
use orchester_exec::programs::Programs;
use orchester_workspace::{
  messages::Message,
  tasks::changes::{
    ChangesError, Checkpointed, NotCheckpointed, ReadChanges, Refresh, RunStage, listed_files, not_checkpointed_in,
  },
};

const HEAD: &str = "1111111111111111111111111111111111111111";

fn revision() -> Revision {
  Revision::new(HEAD).expect("a revision")
}

/// A diff that adds one line to each of `paths`.
fn changes(paths: &[&str]) -> Arc<ReadChanges> {
  let text: String = paths
    .iter()
    .map(|path| format!("diff --git a/{path} b/{path}\n--- a/{path}\n+++ b/{path}\n@@ -1 +1,2 @@\n a\n+b\n"))
    .collect();
  Arc::new(ReadChanges {
    from: revision(),
    diff: Arc::new(Diff::parse(&text).expect("a diff")),
    key: paths.join(",").into(),
  })
}

fn checkpointed(paths: &[&str]) -> Checkpointed {
  Checkpointed::Read {
    head: revision(),
    changes: changes(paths),
  }
}

fn worktree(paths: &[&str]) -> NotCheckpointed {
  NotCheckpointed::Read(changes(paths))
}

fn too_large(files: u32) -> Changes {
  Changes {
    files,
    additions: 30_000,
    deletions: 0,
  }
}

fn count(checkpointed: &Result<Checkpointed, Message>, worktree: &Result<NotCheckpointed, Message>) -> Option<usize> {
  listed_files(Some(checkpointed.as_ref()), Some(worktree.as_ref()))
}

#[test]
fn count_adds_the_files_not_checkpointed_and_counts_a_file_in_both_once() {
  let committed = Ok(checkpointed(&["README.md"]));
  assert_eq!(count(&committed, &Ok(worktree(&["notes.md", "todo.md"]))), Some(3));
  assert_eq!(count(&committed, &Ok(worktree(&["README.md", "notes.md"]))), Some(2));
  assert_eq!(count(&committed, &Ok(NotCheckpointed::Clean)), Some(1));
  assert_eq!(count(&committed, &Ok(NotCheckpointed::Removed)), Some(1));
}

#[test]
fn count_is_unknown_while_either_part_is_read() {
  let committed = Ok(checkpointed(&["README.md"]));
  let uncommitted = Ok(worktree(&["notes.md"]));
  assert_eq!(listed_files(None, Some(uncommitted.as_ref())), None);
  assert_eq!(listed_files(Some(committed.as_ref()), None), None);
}

#[test]
fn count_is_unknown_when_a_read_failed() {
  let failure = Message::new("runs.changes.failure.git");
  assert_eq!(count(&Ok(checkpointed(&["README.md"])), &Err(failure.clone())), None);
  assert_eq!(count(&Err(failure), &Ok(worktree(&["notes.md"]))), None);
}

#[test]
fn count_is_unknown_when_the_worktree_is_too_large_to_list() {
  let large = Ok(NotCheckpointed::TooLarge {
    from: revision(),
    size: too_large(400),
  });
  assert_eq!(count(&Ok(checkpointed(&["README.md"])), &large), None);
}

#[test]
fn count_is_unknown_before_the_first_checkpoint_with_a_clean_worktree() {
  assert_eq!(
    count(&Ok(Checkpointed::NoCheckpoint), &Ok(NotCheckpointed::Clean)),
    None
  );
  assert_eq!(
    count(&Ok(Checkpointed::NoCheckpoint), &Ok(worktree(&["README.md"]))),
    Some(1)
  );
}

#[test]
fn count_of_checkpointed_changes_too_large_to_show_is_the_files_measured() {
  let large = Ok(Checkpointed::TooLarge {
    head: revision(),
    size: too_large(312),
  });
  assert_eq!(count(&large, &Ok(worktree(&["notes.md"]))), Some(312));
  assert_eq!(listed_files(Some(large.as_ref()), None), Some(312));
}

/// Follows `refresh` while `live`, counting reads; what each read returns is `values`' next one.
fn follow(
  runtime: &tokio::runtime::Runtime,
  refresh: &Refresh,
  live: bool,
  values: Vec<u32>,
) -> (
  Arc<AtomicUsize>,
  Arc<parking_lot::Mutex<Vec<u32>>>,
  tokio::task::JoinHandle<()>,
) {
  let (reads, emitted) = (
    Arc::new(AtomicUsize::new(0)),
    Arc::new(parking_lot::Mutex::new(Vec::new())),
  );
  let (counter, sink) = (reads.clone(), emitted.clone());
  let read = move || {
    let index = counter.fetch_add(1, Ordering::SeqCst);
    let value = values[index.min(values.len() - 1)];
    async move { value }
  };
  let task = runtime.spawn(refresh.follower().follow(live, read, move |value| {
    sink.lock().push(value);
    true
  }));
  (reads, emitted, task)
}

fn runtime() -> tokio::runtime::Runtime {
  tokio::runtime::Builder::new_multi_thread()
    .worker_threads(1)
    .enable_all()
    .build()
    .expect("a runtime")
}

/// Waits until `done`, or panics after a few seconds.
fn wait_until(what: &str, done: impl Fn() -> bool) {
  for _ in 0..200 {
    if done() {
      return;
    }
    std::thread::sleep(Duration::from_millis(10));
  }
  panic!("{what}");
}

#[test]
fn worktree_reads_follow_the_ticks_and_emit_only_what_changed() {
  let runtime = runtime();
  let refresh = Refresh::default();
  let tick = refresh.ticker();
  let (reads, emitted, task) = follow(&runtime, &refresh, true, vec![1, 1, 2]);
  wait_until("the first read", || reads.load(Ordering::SeqCst) == 1);
  tick();
  wait_until("the second read", || reads.load(Ordering::SeqCst) == 2);
  tick();
  wait_until("the third read", || reads.load(Ordering::SeqCst) == 3);
  wait_until("the change emitted", || emitted.lock().len() == 2);
  assert_eq!(*emitted.lock(), vec![1, 2]);
  assert!(!task.is_finished());
  drop(refresh);
  wait_until("the reads to end with their page", || task.is_finished());
}

#[test]
fn worktree_reads_end_when_the_page_is_gone_and_its_ticks_wake_nothing() {
  let runtime = runtime();
  let refresh = Refresh::default();
  let tick = refresh.ticker();
  let (reads, _, task) = follow(&runtime, &refresh, true, vec![1]);
  wait_until("the first read", || reads.load(Ordering::SeqCst) == 1);
  drop(refresh);
  wait_until("the reads to end", || task.is_finished());
  // The page's interval may tick once more; nothing reads.
  tick();
  std::thread::sleep(Duration::from_millis(50));
  assert_eq!(reads.load(Ordering::SeqCst), 1);
}

#[test]
fn worktree_is_read_once_while_no_agent_works() {
  let runtime = runtime();
  let refresh = Refresh::default();
  let (reads, _, task) = follow(&runtime, &refresh, false, vec![1]);
  wait_until("the one read to end", || task.is_finished());
  refresh.ticker()();
  std::thread::sleep(Duration::from_millis(50));
  assert_eq!(reads.load(Ordering::SeqCst), 1);
}

#[test]
fn a_missing_worktree_is_not_made_yet_removed_or_an_error_by_the_runs_stage() {
  let repo = tempfile::tempdir().expect("a temporary folder");
  let programs = Programs::processes();
  let read = |stage| not_checkpointed_in(repo.path(), &programs, (RunId::generate(), stage));
  assert!(matches!(read(RunStage::NotStarted), Ok(NotCheckpointed::Clean)));
  assert!(matches!(read(RunStage::Ended), Ok(NotCheckpointed::Removed)));
  let missing = read(RunStage::Started).expect_err("a started run's worktree is there");
  assert!(matches!(missing, ChangesError::WorktreeMissing(_)), "{missing}");
  assert_eq!(missing.message().key, "runs.changes.failure.worktreeMissing");
}
