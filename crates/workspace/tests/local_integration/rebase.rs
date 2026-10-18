//! A run's branch rebased onto newer main after it started (ORC-39): main's own commits between the
//! run's base and the new one are not the run's work integrated, and the run merges; commits of the
//! run's own that main has still show as integrated.

use std::{fs, path::Path};

use orchester_core::integration::local::{RebaseStore, ResultEvidenceKind};
use orchester_exec::programs::Programs;
use orchester_store::ProjectDb;
use orchester_workspace::tasks::{integration::merge_in, local_integration::external_integration};

use super::support::{Fixture, git};

/// Commits `file` with `content` in `root`; the commit.
fn commit(root: &Path, file: &str, content: &str, message: &str) -> String {
  fs::write(root.join(file), content).expect("write");
  git(root, &["add", file]);
  git(root, &["commit", "-m", message]);
  git(root, &["rev-parse", "HEAD"])
}

#[test]
fn local_integration_a_run_rebased_onto_newer_main_is_not_integrated_and_merges() {
  let fixture = Fixture::recorded(false, "rebase-", "task", Some("delivered task\n"));
  let checkpoint = git(&fixture.task_root, &["rev-parse", "HEAD"]);
  let onto = commit(fixture.root(), "later.txt", "later main work\n", "later main work");
  git(&fixture.task_root, &["rebase", "main"]);
  let tip = git(&fixture.task_root, &["rev-parse", "HEAD"]);
  assert_ne!(tip, checkpoint, "the rebase rewrote the checkpointed commit");
  assert_eq!(
    external_integration(fixture.root(), fixture.run).expect("evidence"),
    None,
    "main's own commit is not the run's work"
  );
  let rebase = ProjectDb::open(fixture.root())
    .expect("db")
    .run_rebase(fixture.run)
    .expect("read")
    .expect("the rebase is recorded");
  assert_eq!(rebase.base().as_str(), onto);
  let rewritten = rebase.checkpoint().expect("the checkpoint was rewritten");
  assert_eq!(
    (rewritten.replaces.as_str(), rewritten.tip.as_str()),
    (checkpoint.as_str(), tip.as_str())
  );
  let merged = merge_in(fixture.root(), &Programs::processes(), (fixture.run, None)).expect("merged");
  assert_eq!(merged.merge.revision.as_str(), tip);
  assert_eq!(git(fixture.root(), &["rev-parse", "main"]), tip);
}

#[test]
fn local_integration_a_run_rebased_without_checkpoints_is_read_from_its_new_base() {
  let fixture = Fixture::new(false);
  let onto = commit(fixture.root(), "later.txt", "later main work\n", "later main work");
  git(&fixture.task_root, &["rebase", "main"]);
  assert_eq!(
    external_integration(fixture.root(), fixture.run).expect("evidence"),
    None
  );
  let rebase = ProjectDb::open(fixture.root())
    .expect("db")
    .run_rebase(fixture.run)
    .expect("read")
    .expect("recorded");
  assert_eq!(rebase.base().as_str(), onto);
  assert_eq!(rebase.checkpoint(), None);
}

#[test]
fn local_integration_a_commit_of_the_run_cherry_picked_into_main_still_shows() {
  let fixture = Fixture::new(false);
  let first = git(&fixture.task_root, &["rev-parse", "HEAD"]);
  commit(&fixture.task_root, "more.txt", "more task work\n", "more task work");
  // Main moved first, so the pick is a commit of its own rather than the run's again.
  commit(
    fixture.root(),
    "later.txt",
    "later main work
",
    "later main work",
  );
  git(fixture.root(), &["cherry-pick", &first]);
  let evidence = external_integration(fixture.root(), fixture.run)
    .expect("evidence")
    .expect("part of the run is in main");
  assert_eq!(evidence.kind, ResultEvidenceKind::EquivalentPatches);
  assert!(
    ProjectDb::open(fixture.root())
      .expect("db")
      .run_rebase(fixture.run)
      .expect("read")
      .is_none()
  );
}

#[test]
fn local_integration_a_rebase_onto_main_that_holds_a_checkpoint_of_the_run_moves_no_base() {
  let fixture = Fixture::recorded(false, "rebase-own-", "task", Some("delivered task\n"));
  git(fixture.root(), &["merge", "--ff-only", "task"]);
  commit(fixture.root(), "later.txt", "later main work\n", "later main work");
  commit(&fixture.task_root, "more.txt", "more task work\n", "more task work");
  git(&fixture.task_root, &["rebase", "main"]);
  assert!(
    external_integration(fixture.root(), fixture.run)
      .expect("evidence")
      .is_some(),
    "the run's checkpointed commit is in main"
  );
  assert!(
    ProjectDb::open(fixture.root())
      .expect("db")
      .run_rebase(fixture.run)
      .expect("read")
      .is_none(),
    "no base was moved over the run's own work"
  );
}

/// The reviewer's probe: a checkpoint that indents `return 1` under `if y:` and a tip that does
/// not are different patches, though `git cherry` takes them as one.
#[test]
fn local_integration_a_rebase_that_changed_whitespace_of_the_checkpoint_does_not_stand_for_it() {
  let fixture = Fixture::recorded(false, "rebase-space-", "task", Some("if y:\n        return 1\n"));
  let checkpoint = git(&fixture.task_root, &["rev-parse", "HEAD"]);
  commit(fixture.root(), "later.txt", "later main work\n", "later main work");
  git(&fixture.task_root, &["rebase", "main"]);
  fs::write(fixture.task_root.join("task.txt"), "if y:\nreturn 1\n").expect("dedent");
  git(&fixture.task_root, &["commit", "--amend", "--all", "--no-edit"]);
  let evidence = external_integration(fixture.root(), fixture.run)
    .expect("evidence")
    .expect("the checkpoint is not the tip");
  assert_eq!(evidence.kind, ResultEvidenceKind::UnknownSource);
  assert_eq!(evidence.source.as_str(), checkpoint);
  let rebase = ProjectDb::open(fixture.root())
    .expect("db")
    .run_rebase(fixture.run)
    .expect("read")
    .expect("the rebase itself is proven");
  assert_eq!(rebase.checkpoint(), None, "the tip does not stand for the checkpoint");
}
