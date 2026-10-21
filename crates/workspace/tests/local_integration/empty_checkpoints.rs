//! A run whose agent merged main as the base-moved notice asks (ORC-54): an empty `checkpoint before merging main`
//! commit, then `git merge main`, while main already holds such an empty commit of another run merged before. Every
//! empty commit has the same patch id, so main seemed to hold part of the run's work and the run could not be merged:
//! not through *Merge into main*, `merge_run`, nor the Local integration page. Main's commits and empty commits are
//! not the run's result; its own commits main holds, merged or cherry-picked, are still found there.

use std::{fs, path::Path};

use orchester_core::{integration::local::ResultEvidenceKind, store::RunStore};
use orchester_exec::programs::Programs;
use orchester_store::ProjectDb;
use orchester_workspace::tasks::{
  delivery::{self, Delivery},
  integration::merge_in,
  local_integration::{IntegrationError, LocalIntegration, external_integration, read},
};

use super::{
  privacy::Vault,
  support::{Fixture, git},
};

/// Commits `file` with `content` in `root`; the commit.
fn commit(root: &Path, file: &str, content: &str, message: &str) -> String {
  fs::write(root.join(file), content).expect("write");
  git(root, &["add", file]);
  git(root, &["commit", "-m", message]);
  git(root, &["rev-parse", "HEAD"])
}

/// Another run's empty `checkpoint before merging main` commit merged into main, as `Merge ORC-67 run #42` did.
fn land_empty_checkpoint(root: &Path) {
  let tree = git(root, &["rev-parse", "main^{tree}"]);
  let main = git(root, &["rev-parse", "main"]);
  let empty = git(
    root,
    &[
      "commit-tree",
      &tree,
      "-p",
      &main,
      "-m",
      "checkpoint before merging main",
    ],
  );
  git(root, &["merge", "--no-ff", "--no-edit", &empty]);
}

/// The agent's merge of main as the base-moved notice asks for it: a commit of its state, empty when it has
/// nothing to commit, then a merge.
fn merge_main(task: &Path) {
  git(
    task,
    &["commit", "--allow-empty", "-m", "checkpoint before merging main"],
  );
  git(task, &["merge", "--no-ff", "--no-edit", "main"]);
}

/// ORC-63 run #40: a checkpointed commit, after which main moved and the agent merged main; main moves on after and
/// takes another run's empty checkpoint commit, one the run's branch does not hold. The checkpointed commit.
fn merged_once(prefix: &str) -> (Fixture, String) {
  let mut first = String::new();
  let fixture = Fixture::worked(false, (prefix, "task"), |root, task| {
    first = commit(task, "task.txt", "delivered task\n", "task work");
    commit(root, "landed.txt", "landed work\n", "ORC-51: landed");
    merge_main(task);
    vec![first.clone()]
  });
  commit(fixture.root(), "later.txt", "later main work\n", "later main work");
  land_empty_checkpoint(fixture.root());
  (fixture, first)
}

/// ORC-56 run #35: merged main twice this way, with a checkpointed commit between; main moves on after and takes
/// another run's empty checkpoint commit. The first checkpointed commit.
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
  land_empty_checkpoint(fixture.root());
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
fn empty_checkpoints_a_run_that_merged_main_after_one_merges_with_one_click() {
  let (fixture, _) = merged_once("empty-checkpoint-");
  assert_eq!(
    external_integration(fixture.root(), fixture.run).expect("evidence"),
    None,
    "an empty commit of main is not the run's work"
  );
  assert_eq!(delivery(&fixture), Delivery::Waiting);
  let tip = git(fixture.root(), &["rev-parse", "task"]);
  merge_in(fixture.root(), &Programs::processes(), (fixture.run, None)).expect("merged");
  assert_eq!(git(fixture.root(), &["merge-base", "--is-ancestor", &tip, "main"]), "");
  assert_eq!(git(fixture.root(), &["show", "main:task.txt"]), "delivered task");
  assert_eq!(git(fixture.root(), &["show", "main:later.txt"]), "later main work");
}

#[test]
fn empty_checkpoints_the_local_integration_page_opens_such_a_run_with_no_evidence_against_it() {
  let (fixture, _) = merged_once("empty-checkpoint-page-");
  let view = read(fixture.root(), fixture.run, None, &Vault(None)).expect("review");
  assert_eq!(view.external, None, "the page shows nothing to reconcile");
  let session = LocalIntegration::open(fixture.root(), fixture.run).expect("the page opens the run");
  assert!(session.operation().merged().is_none());
}

#[test]
fn empty_checkpoints_a_run_that_merged_main_twice_merges() {
  let (fixture, _) = merged_twice("empty-checkpoint-twice-");
  assert_eq!(
    external_integration(fixture.root(), fixture.run).expect("evidence"),
    None
  );
  merge_in(fixture.root(), &Programs::processes(), (fixture.run, None)).expect("merged");
  assert_eq!(git(fixture.root(), &["show", "main:task.txt"]), "delivered task");
  assert_eq!(git(fixture.root(), &["show", "main:more.txt"]), "more task work");
}

#[test]
fn empty_checkpoints_a_run_main_merged_outside_reads_as_merged() {
  let (fixture, _) = merged_once("empty-checkpoint-outside-");
  git(fixture.root(), &["merge", "--no-ff", "--no-edit", "task"]);
  let evidence = external_integration(fixture.root(), fixture.run)
    .expect("evidence")
    .expect("the run is in main");
  assert_eq!(evidence.kind, ResultEvidenceKind::CompleteAncestry);
  assert_eq!(delivery(&fixture), Delivery::MergedOutside("main".into()));
  assert!(matches!(
    LocalIntegration::open(fixture.root(), fixture.run),
    Err(IntegrationError::AlreadyIntegrated)
  ));
}

#[test]
fn empty_checkpoints_a_cherry_picked_own_commit_still_needs_local_integration() {
  let (fixture, first) = merged_once("empty-checkpoint-pick-");
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
fn empty_checkpoints_an_own_commit_merged_into_main_still_needs_local_integration() {
  let (fixture, first) = merged_twice("empty-checkpoint-partial-");
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
fn empty_checkpoints_a_commit_not_made_on_the_branch_is_still_unknown() {
  let (fixture, _) = merged_once("empty-checkpoint-foreign-");
  let tip = git(fixture.root(), &["rev-parse", "task"]);
  let tree = git(fixture.root(), &["rev-parse", "task^{tree}"]);
  let foreign = git(
    fixture.root(),
    &["commit-tree", &tree, "-p", &tip, "-m", "made elsewhere"],
  );
  git(fixture.root(), &["update-ref", "refs/heads/task", &foreign]);
  let evidence = external_integration(fixture.root(), fixture.run)
    .expect("evidence")
    .expect("the branch no longer stands for the checkpoint");
  assert_eq!(evidence.kind, ResultEvidenceKind::UnknownSource);
  let before = git(fixture.root(), &["rev-parse", "main"]);
  assert!(merge_in(fixture.root(), &Programs::processes(), (fixture.run, None)).is_err());
  assert_eq!(git(fixture.root(), &["rev-parse", "main"]), before, "nothing merged");
}
