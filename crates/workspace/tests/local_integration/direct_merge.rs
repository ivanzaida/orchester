//! The direct merge (*Merge into main*, `merge_run`) beside the person's uncommitted work: a local integration that
//! only reviewed the checkout, also one paused since, does not stand in its way; one that started work does, and
//! Local integration is where the merge carries on, as it is for uncommitted files the merge would overwrite.

use std::fs;

use orchester_core::{
  integration::{
    MergeError, MergeKind, RunMerge,
    local::{LocalIntegrationStore, LocalPhase, LocalProcessStore},
  },
  revision::Revision,
  run::BranchName,
  store::MergeStore,
  time::Timestamp,
};
use orchester_exec::programs::Programs;
use orchester_store::ProjectDb;
use orchester_workspace::tasks::{
  integration::{IntegrationProblem, merge_in},
  local_integration::CheckoutHolder,
};

use super::support::{Fixture, git};

/// The person's work beside the run's: `local.txt` changed and not staged, `notes.txt` untracked.
fn person_work(fixture: &Fixture) -> Vec<(&'static str, Vec<u8>)> {
  fs::write(fixture.root().join("local.txt"), "base\nthe person's edit\n").expect("edit");
  fs::write(fixture.root().join("notes.txt"), "untracked notes\n").expect("untracked");
  ["local.txt", "notes.txt"]
    .into_iter()
    .map(|path| (path, fs::read(fixture.root().join(path)).expect("bytes")))
    .collect()
}

fn unchanged(fixture: &Fixture, work: &[(&str, Vec<u8>)]) {
  for (path, bytes) in work {
    assert_eq!(&fs::read(fixture.root().join(path)).expect("bytes"), bytes, "{path}");
  }
}

#[test]
fn local_integration_direct_merge_passes_a_review_paused_before_it_held_anything() {
  let fixture = Fixture::new(false);
  let work = person_work(&fixture);
  let mut session = fixture.open();
  session.pause().expect("pause the review");
  drop(session);
  let db = ProjectDb::open(fixture.root()).expect("db");
  let paused = db.local_operation(fixture.run).expect("read").expect("the review");
  assert_eq!(paused.phase(), LocalPhase::Paused);
  assert!(!paused.reserves_checkout());
  let source = git(&fixture.task_root, &["rev-parse", "HEAD"]);
  let merged = merge_in(fixture.root(), &Programs::processes(), (fixture.run, None)).expect("direct merge");
  assert_eq!(merged.merge.kind, MergeKind::FastForward);
  assert_eq!(git(fixture.root(), &["rev-parse", "HEAD"]), source);
  unchanged(&fixture, &work);
  assert_eq!(
    git(fixture.root(), &["status", "--porcelain"]),
    "M local.txt\n?? notes.txt",
    "the support's git trims the first line's leading space"
  );
  assert!(db.run_merge(fixture.run).expect("record").is_some());
  assert!(
    merged.cleanup.is_ok(),
    "the paused review does not keep the run's worktree"
  );
  let settled = db.local_operation(fixture.run).expect("read").expect("the review");
  assert_eq!(settled.phase(), LocalPhase::Complete, "the review reads as merged");
  assert_eq!(settled.merged(), Some(merged.merge.revision.as_str()));
}

#[test]
fn local_integration_direct_merge_waits_for_this_runs_local_integration_that_started_work() {
  let fixture = Fixture::new(false);
  fs::write(fixture.root().join("local.txt"), "base\nthe person's edit\n").expect("edit");
  let mut session = fixture.open();
  session.preserve("Before the merge").expect("preserve");
  drop(session);
  let before = git(fixture.root(), &["rev-parse", "HEAD"]);
  let refused = merge_in(fixture.root(), &Programs::processes(), (fixture.run, None)).expect_err("held");
  assert_eq!(refused, IntegrationProblem::HeldLocally(CheckoutHolder::ThisRun));
  assert!(refused.continues_locally());
  assert_eq!(git(fixture.root(), &["rev-parse", "HEAD"]), before);
}

#[test]
fn local_integration_direct_merge_at_a_dirty_path_changes_nothing_and_continues_locally() {
  let fixture = Fixture::new(false);
  fs::write(fixture.root().join("task.txt"), "the person's task edit\n").expect("edit");
  let bytes = fs::read(fixture.root().join("task.txt")).expect("bytes");
  let before = git(fixture.root(), &["rev-parse", "HEAD"]);
  let refused = merge_in(fixture.root(), &Programs::processes(), (fixture.run, None)).expect_err("in the way");
  assert_eq!(
    refused,
    IntegrationProblem::Merge(MergeError::UncommittedInTheWay {
      files: vec!["task.txt".to_owned()]
    })
  );
  assert!(refused.continues_locally());
  assert_eq!(git(fixture.root(), &["rev-parse", "HEAD"]), before);
  assert_eq!(fs::read(fixture.root().join("task.txt")).expect("bytes"), bytes);
}

/// A local integration that preserved the person's work, was canceled (the originals put back) and had its kept
/// originals removed: it holds nothing.
fn canceled_and_cleaned(fixture: &Fixture) {
  fs::write(fixture.root().join("local.txt"), "base\nthe person's edit\n").expect("edit");
  let mut session = fixture.open();
  session.preserve("Before the merge").expect("preserve");
  session.cancel_preservation().expect("put the originals back");
  session.remove_preservation().expect("remove the kept originals");
  drop(session);
  let db = ProjectDb::open(fixture.root()).expect("db");
  let canceled = db.local_operation(fixture.run).expect("read").expect("the record");
  assert_eq!(canceled.phase(), LocalPhase::Canceled);
  assert!(!canceled.preservation().expect("the preservation").retained);
  assert!(!canceled.reserves_checkout());
}

#[test]
fn local_integration_direct_merge_beside_a_canceled_cleaned_integration_completes_it() {
  let fixture = Fixture::new(false);
  canceled_and_cleaned(&fixture);
  let merged = merge_in(fixture.root(), &Programs::processes(), (fixture.run, None)).expect("direct merge");
  let db = ProjectDb::open(fixture.root()).expect("db");
  let settled = db.local_operation(fixture.run).expect("read").expect("the record");
  assert_eq!(settled.phase(), LocalPhase::Complete, "nothing is left to restore");
  assert_eq!(settled.merged(), Some(merged.merge.revision.as_str()));
  assert!(!settled.reserves_checkout(), "the project's next merge is not held");
  assert_eq!(
    fs::read_to_string(fixture.root().join("local.txt")).expect("the person's file"),
    "base\nthe person's edit\n"
  );
}

#[test]
fn local_integration_opening_a_canceled_cleaned_integration_after_a_merge_completes_it() {
  let fixture = Fixture::new(false);
  canceled_and_cleaned(&fixture);
  // Merged without the direct merge's settling: git by hand, then the record, as another window would.
  git(fixture.root(), &["merge", "--ff-only", "task"]);
  let head = git(fixture.root(), &["rev-parse", "HEAD"]);
  let mut db = ProjectDb::open(fixture.root()).expect("db");
  db.save_run_merge(&RunMerge {
    run: fixture.run,
    into: BranchName::new("main").expect("branch"),
    revision: Revision::new(&head).expect("revision"),
    kind: MergeKind::FastForward,
    merged_at: Timestamp::now(),
  })
  .expect("record the merge");
  let session = fixture.open();
  assert_eq!(session.operation().phase(), LocalPhase::Complete);
  assert!(!session.operation().reserves_checkout());
}

#[test]
fn local_integration_direct_merge_waits_for_an_unfinished_git_process_of_a_review() {
  let fixture = Fixture::new(false);
  let mut session = fixture.open();
  session.pause().expect("pause the review");
  drop(session);
  let mut db = ProjectDb::open(fixture.root()).expect("db");
  db.begin_local_process(fixture.run).expect("a git process begins");
  let refused = merge_in(fixture.root(), &Programs::processes(), (fixture.run, None)).expect_err("held");
  assert_eq!(refused, IntegrationProblem::HeldLocally(CheckoutHolder::ThisRun));
}
