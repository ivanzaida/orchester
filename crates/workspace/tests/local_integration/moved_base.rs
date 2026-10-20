//! A run whose branch took newer main in after it started (ORC-60): fast-forwarded onto it, as an agent told to merge
//! the base before it had committed anything does, or merged it with `--no-ff`. Main's commits are not the run's
//! work integrated: the run merges, and its changes are its own work only, from the commit it took in, while it keeps
//! the revision it started from. Commits of the run's own that main has, by a cherry-pick or a manual merge of part of
//! the run, still need Local integration.

use std::{fs, path::Path};

use orchester_core::{integration::local::ResultEvidenceKind, store::RunStore};
use orchester_exec::programs::Programs;
use orchester_store::ProjectDb;
use orchester_workspace::tasks::{
  changes::{Checkpointed, RunRange, run_changes_in},
  integration::merge_in,
  local_integration::external_integration,
};

use super::support::{Fixture, git};

/// Commits `file` with `content` in `root`; the commit.
fn commit(root: &Path, file: &str, content: &str, message: &str) -> String {
  fs::write(root.join(file), content).expect("write");
  git(root, &["add", file]);
  git(root, &["commit", "-m", message]);
  git(root, &["rev-parse", "HEAD"])
}

/// Main lands a task's work, and the run's branch, with nothing committed yet, fast-forwards onto it.
fn fast_forward(root: &Path, task: &Path) -> String {
  let landed = commit(root, "landed.txt", "landed work\n", "ORC-51: landed");
  git(task, &["merge", "main"]);
  assert_eq!(git(task, &["rev-parse", "HEAD"]), landed, "the branch fast-forwarded");
  landed
}

/// The files of the run's changes, as Changes and *What changed* read them, and the commit they start from.
fn changes(fixture: &Fixture) -> (Vec<String>, String) {
  let run = ProjectDb::open(fixture.root())
    .expect("db")
    .run(fixture.run)
    .expect("read")
    .expect("run");
  match run_changes_in(fixture.root(), &Programs::processes(), &RunRange::of(&run)).expect("changes") {
    Checkpointed::Read { changes, .. } => (
      changes.diff.files().iter().map(|file| file.path().to_owned()).collect(),
      changes.from.to_string(),
    ),
    other => panic!("the changes are read: {other:?}"),
  }
}

fn recorded_base(fixture: &Fixture) -> String {
  ProjectDb::open(fixture.root())
    .expect("db")
    .run(fixture.run)
    .expect("read")
    .expect("run")
    .base()
    .to_string()
}

#[test]
fn moved_base_a_run_fast_forwarded_onto_newer_main_merges_and_shows_only_its_own_work() {
  let fixture = Fixture::worked(false, ("moved-ff-", "task"), |root, task| {
    fast_forward(root, task);
    vec![commit(task, "task.txt", "delivered task\n", "task work")]
  });
  let started = git(fixture.root(), &["rev-parse", "main~1"]);
  let landed = git(fixture.root(), &["rev-parse", "main"]);
  // Main moved on since, as master had in ORC-55.
  commit(fixture.root(), "later.txt", "later main work\n", "later main work");
  assert_eq!(
    external_integration(fixture.root(), fixture.run).expect("evidence"),
    None,
    "main's landed commit is not the run's work"
  );
  assert_eq!(
    changes(&fixture),
    (vec!["task.txt".to_owned()], landed),
    "only the run's own work"
  );
  assert_eq!(recorded_base(&fixture), started, "the start revision is kept");
  merge_in(fixture.root(), &Programs::processes(), (fixture.run, None)).expect("merged");
  assert_eq!(git(fixture.root(), &["show", "main:task.txt"]), "delivered task");
  assert_eq!(git(fixture.root(), &["show", "main:later.txt"]), "later main work");
}

#[test]
fn moved_base_a_run_that_merged_main_with_no_ff_after_a_checkpoint_merges() {
  let fixture = Fixture::worked(false, ("moved-no-ff-", "task"), |root, task| {
    let first = commit(task, "task.txt", "delivered task\n", "task work");
    commit(root, "landed.txt", "landed work\n", "ORC-51: landed");
    git(
      task,
      &["commit", "--allow-empty", "-m", "checkpoint before merging main"],
    );
    let checkpoint = git(task, &["rev-parse", "HEAD"]);
    git(task, &["merge", "--no-ff", "--no-edit", "main"]);
    let more = commit(task, "more.txt", "more task work\n", "more task work");
    vec![first, checkpoint, more]
  });
  let landed = git(fixture.root(), &["rev-parse", "main"]);
  commit(fixture.root(), "later.txt", "later main work\n", "later main work");
  assert_eq!(
    external_integration(fixture.root(), fixture.run).expect("evidence"),
    None
  );
  let (files, from) = changes(&fixture);
  assert_eq!(files, ["more.txt", "task.txt"], "landed.txt is main's");
  assert_eq!(from, landed);
  merge_in(fixture.root(), &Programs::processes(), (fixture.run, None)).expect("merged");
  assert_eq!(git(fixture.root(), &["show", "main:more.txt"]), "more task work");
}

#[test]
fn moved_base_a_commit_of_the_run_cherry_picked_into_main_still_needs_local_integration() {
  let fixture = Fixture::worked(false, ("moved-pick-", "task"), |root, task| {
    fast_forward(root, task);
    let first = commit(task, "task.txt", "delivered task\n", "task work");
    let more = commit(task, "more.txt", "more task work\n", "more task work");
    commit(root, "later.txt", "later main work\n", "later main work");
    git(root, &["cherry-pick", &first]);
    vec![first, more]
  });
  let evidence = external_integration(fixture.root(), fixture.run)
    .expect("evidence")
    .expect("part of the run is in main");
  assert_eq!(evidence.kind, ResultEvidenceKind::EquivalentPatches);
  let before = git(fixture.root(), &["rev-parse", "main"]);
  assert!(merge_in(fixture.root(), &Programs::processes(), (fixture.run, None)).is_err());
  assert_eq!(git(fixture.root(), &["rev-parse", "main"]), before, "nothing merged");
}

#[test]
fn moved_base_a_partial_manual_merge_of_the_run_still_needs_local_integration() {
  let fixture = Fixture::worked(false, ("moved-partial-", "task"), |root, task| {
    fast_forward(root, task);
    let first = commit(task, "task.txt", "delivered task\n", "task work");
    let more = commit(task, "more.txt", "more task work\n", "more task work");
    git(root, &["merge", "--no-ff", "--no-edit", &first]);
    vec![first, more]
  });
  let evidence = external_integration(fixture.root(), fixture.run)
    .expect("evidence")
    .expect("part of the run is in main");
  assert_eq!(evidence.kind, ResultEvidenceKind::PartialAncestry);
  let before = git(fixture.root(), &["rev-parse", "main"]);
  assert!(merge_in(fixture.root(), &Programs::processes(), (fixture.run, None)).is_err());
  assert_eq!(git(fixture.root(), &["rev-parse", "main"]), before, "nothing merged");
}

#[test]
fn moved_base_main_fast_forwarded_onto_an_unrecorded_commit_of_the_run_still_needs_local_integration() {
  // No checkpoint is recorded: the branch's reflog alone shows the run made the commit main took.
  let fixture = Fixture::worked(false, ("moved-unrecorded-", "task"), |root, task| {
    let first = commit(task, "task.txt", "delivered task\n", "task work");
    commit(task, "more.txt", "more task work\n", "more task work");
    git(root, &["merge", "--ff-only", &first]);
    Vec::new()
  });
  let evidence = external_integration(fixture.root(), fixture.run)
    .expect("evidence")
    .expect("part of the run is in main");
  assert_eq!(evidence.kind, ResultEvidenceKind::PartialAncestry);
}

#[test]
fn moved_base_a_merged_run_still_shows_its_own_work_from_the_commit_it_took_in() {
  let fixture = Fixture::worked(false, ("moved-merged-", "task"), |root, task| {
    fast_forward(root, task);
    vec![commit(task, "task.txt", "delivered task\n", "task work")]
  });
  let landed = git(fixture.root(), &["rev-parse", "main"]);
  merge_in(fixture.root(), &Programs::processes(), (fixture.run, None)).expect("merged");
  assert_eq!(changes(&fixture), (vec!["task.txt".to_owned()], landed));
}
