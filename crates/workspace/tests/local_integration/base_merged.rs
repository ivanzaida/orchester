//! A run whose agent merged main into its branch itself after its last checkpoint, as one that follows the base-moved
//! notice or resolves a conflict does (ORC-67): the merge, and the commit before it, record no checkpoint. Main's
//! commits are not the run's result, and the commits its agent made are: the run merges with one click. Its own commits
//! main has, merged or cherry-picked, are still found in main.

use std::{fs, path::Path};

use orchester_core::{integration::local::ResultEvidenceKind, store::RunStore};
use orchester_exec::programs::Programs;
use orchester_store::ProjectDb;
use orchester_workspace::tasks::{
  delivery::{self, Delivery},
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

/// The agent's merge of main as the base-moved notice asks for it: a commit of its state, then a merge.
fn merge_main(task: &Path) {
  git(
    task,
    &["commit", "--allow-empty", "-m", "checkpoint before merging main"],
  );
  git(task, &["merge", "--no-ff", "--no-edit", "main"]);
}

/// A run that checkpointed its first commit and then, twice, merged main after main moved, the second time after its
/// last checkpoint; main moves on after. Its first commit.
fn merged_twice(prefix: &str) -> (Fixture, String) {
  let mut first = String::new();
  let fixture = Fixture::worked(false, (prefix, "task"), |root, task| {
    first = commit(task, "task.txt", "delivered task\n", "task work");
    commit(root, "landed.txt", "landed work\n", "ORC-51: landed");
    merge_main(task);
    let more = commit(task, "more.txt", "more task work\n", "more task work");
    commit(root, "landed-again.txt", "landed again\n", "ORC-52: landed");
    merge_main(task);
    vec![first.clone(), more]
  });
  commit(fixture.root(), "later.txt", "later main work\n", "later main work");
  (fixture, first)
}

fn delivery(fixture: &Fixture) -> Delivery {
  let run = ProjectDb::open(fixture.root())
    .expect("db")
    .run(fixture.run)
    .expect("read")
    .expect("run");
  delivery::of_run(&Programs::processes(), fixture.root(), &run)
}

#[test]
fn base_merged_a_run_that_merged_main_after_its_last_checkpoint_merges_with_one_click() {
  let (fixture, _) = merged_twice("base-merged-");
  assert_eq!(
    external_integration(fixture.root(), fixture.run).expect("evidence"),
    None,
    "main's commits are not the run's result"
  );
  assert_eq!(delivery(&fixture), Delivery::Waiting);
  let tip = git(fixture.root(), &["rev-parse", "task"]);
  merge_in(fixture.root(), &Programs::processes(), (fixture.run, None)).expect("merged");
  assert_eq!(git(fixture.root(), &["merge-base", "--is-ancestor", &tip, "main"]), "");
  assert_eq!(git(fixture.root(), &["show", "main:more.txt"]), "more task work");
  assert_eq!(git(fixture.root(), &["show", "main:later.txt"]), "later main work");
}

#[test]
fn base_merged_a_run_main_merged_outside_reads_as_merged() {
  let (fixture, _) = merged_twice("base-merged-outside-");
  git(fixture.root(), &["merge", "--no-ff", "--no-edit", "task"]);
  let evidence = external_integration(fixture.root(), fixture.run)
    .expect("evidence")
    .expect("the run is in main");
  assert_eq!(evidence.kind, ResultEvidenceKind::CompleteAncestry);
  assert_eq!(delivery(&fixture), Delivery::MergedOutside("main".into()));
}

#[test]
fn base_merged_a_commit_of_the_run_cherry_picked_into_main_still_needs_local_integration() {
  let (fixture, first) = merged_twice("base-merged-pick-");
  git(fixture.root(), &["cherry-pick", &first]);
  let evidence = external_integration(fixture.root(), fixture.run)
    .expect("evidence")
    .expect("part of the run is in main");
  assert_eq!(evidence.kind, ResultEvidenceKind::EquivalentPatches);
  let before = git(fixture.root(), &["rev-parse", "main"]);
  assert!(merge_in(fixture.root(), &Programs::processes(), (fixture.run, None)).is_err());
  assert_eq!(git(fixture.root(), &["rev-parse", "main"]), before, "nothing merged");
}

#[test]
fn base_merged_a_commit_of_the_run_from_before_its_merges_merged_into_main_still_needs_local_integration() {
  let (fixture, first) = merged_twice("base-merged-partial-");
  git(fixture.root(), &["merge", "--no-ff", "--no-edit", &first]);
  let evidence = external_integration(fixture.root(), fixture.run)
    .expect("evidence")
    .expect("part of the run is in main");
  assert_eq!(evidence.kind, ResultEvidenceKind::PartialAncestry);
  let before = git(fixture.root(), &["rev-parse", "main"]);
  assert!(merge_in(fixture.root(), &Programs::processes(), (fixture.run, None)).is_err());
  assert_eq!(git(fixture.root(), &["rev-parse", "main"]), before, "nothing merged");
}

#[test]
fn base_merged_a_run_with_no_merge_of_main_whose_agent_committed_after_its_checkpoint_merges() {
  let fixture = Fixture::worked(false, ("base-merged-plain-", "task"), |_, task| {
    vec![commit(task, "task.txt", "delivered task\n", "task work")]
  });
  commit(&fixture.task_root, "more.txt", "more task work\n", "more task work");
  commit(fixture.root(), "later.txt", "later main work\n", "later main work");
  assert_eq!(
    external_integration(fixture.root(), fixture.run).expect("evidence"),
    None
  );
  merge_in(fixture.root(), &Programs::processes(), (fixture.run, None)).expect("merged");
  assert_eq!(git(fixture.root(), &["show", "main:more.txt"]), "more task work");
}
