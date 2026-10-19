//! Recording a merge made without Orchester as a local integration's own: only where the record stopped at a step
//! that left nothing under way, so what it holds can still be put back or removed afterwards.

use orchester_core::{
  blob::ContentHash,
  id::RunId,
  integration::local::{
    CheckedResult, Checkout, ContainedResult, LocalError, LocalFile, LocalOperation, LocalPath, LocalPhase,
    ResultEvidenceKind,
  },
  revision::Revision,
};

fn checkout(head: char, files: Vec<LocalFile>) -> Checkout {
  Checkout {
    root: String::from("/project"),
    git_dir: String::from("/project/.git"),
    branch: String::from("main"),
    head: head.to_string().repeat(40),
    index: ContentHash::new(&"a".repeat(64)).expect("hash"),
    files,
    ignored: ContentHash::new(&"b".repeat(64)).expect("hash"),
    operation: None,
  }
}

fn edited() -> LocalFile {
  LocalFile {
    path: LocalPath::new("local.txt").expect("path"),
    head: None,
    staged: None,
    working: Some(ContentHash::new(&"c".repeat(64)).expect("hash")),
    executable: false,
    unstaged: false,
  }
}

/// Git's proof that the branch's whole work is in commit `d…`.
fn whole() -> ContainedResult {
  ContainedResult {
    kind: ResultEvidenceKind::CompleteAncestry,
    source: Revision::new(&"e".repeat(40)).expect("revision"),
    target: Revision::new(&"d".repeat(40)).expect("revision"),
  }
}

fn review() -> LocalOperation {
  LocalOperation::new(RunId::generate(), checkout('a', Vec::new())).expect("review")
}

fn refused(mut operation: LocalOperation, at: &str) {
  assert!(!operation.can_record_merged_outside(), "{at}");
  let before = operation.phase();
  assert_eq!(
    operation.record_merged_outside(&whole()),
    Err(LocalError::WrongPhase),
    "{at}"
  );
  assert_eq!(operation.phase(), before, "{at}");
  assert!(operation.merged().is_none(), "{at}");
}

fn recorded(mut operation: LocalOperation, at: &str) -> LocalOperation {
  assert!(operation.can_record_merged_outside(), "{at}");
  operation.record_merged_outside(&whole()).expect(at);
  assert_eq!(operation.merged(), Some("d".repeat(40).as_str()), "{at}");
  assert_eq!(operation.merged_outside(), Some("d".repeat(40).as_str()), "{at}");
  operation
}

#[test]
fn local_merged_outside_is_refused_mid_step() {
  let dirty = checkout('a', vec![edited()]);
  let mut preserving = LocalOperation::new(RunId::generate(), dirty.clone()).expect("review");
  preserving
    .begin_preserve(&dirty, "Before the merge")
    .expect("preserving");
  assert_eq!(preserving.phase(), LocalPhase::Preserving);
  let mut paused = preserving.clone();
  paused.paused();
  refused(preserving, "an unverified preservation");
  refused(paused, "paused while preserving");

  let base = checkout('a', Vec::new());
  let mut syncing = review();
  syncing.prepared(base.clone()).expect("prepared");
  syncing.begin_sync(&base).expect("syncing");
  let mut conflicts = syncing.clone();
  conflicts.task_conflicts().expect("task conflicts");
  refused(syncing, "a sync under way");
  refused(conflicts, "task conflicts");

  let mut canceling = review();
  canceling.begin_cancel(&base).expect("canceling");
  assert_eq!(canceling.phase(), LocalPhase::Canceling);
  refused(canceling, "a cancellation under way");

  let partial = ContainedResult {
    kind: ResultEvidenceKind::PartialAncestry,
    ..whole()
  };
  assert_eq!(review().record_merged_outside(&partial), Err(LocalError::Reconcile));
}

#[test]
fn local_merged_outside_is_recorded_where_the_record_stopped_between_steps() {
  let base = checkout('a', Vec::new());
  let done = recorded(review(), "a review");
  assert_eq!(done.phase(), LocalPhase::Complete);

  let mut prepared = review();
  prepared.prepared(base.clone()).expect("prepared");
  assert_eq!(recorded(prepared.clone(), "prepared").phase(), LocalPhase::Complete);
  let mut paused = prepared;
  paused.paused();
  assert_eq!(recorded(paused, "paused when prepared").phase(), LocalPhase::Complete);

  let dirty = checkout('a', vec![edited()]);
  let mut preserved = LocalOperation::new(RunId::generate(), dirty.clone()).expect("review");
  preserved
    .begin_preserve(&dirty, "Before the merge")
    .expect("preserving");
  preserved.preservation_verified().expect("verified");
  preserved.preservation_finished(base).expect("preserved");
  assert_eq!(
    recorded(preserved, "verified originals").phase(),
    LocalPhase::MergedAwaitingRestore,
    "the originals are put back next"
  );
}

#[test]
fn local_merged_outside_after_a_real_sync_and_check_restores_and_completes() {
  let dirty = checkout('a', vec![edited()]);
  let base = checkout('a', Vec::new());
  let mut operation = LocalOperation::new(RunId::generate(), dirty.clone()).expect("review");
  operation
    .begin_preserve(&dirty, "Before the merge")
    .expect("preserving");
  operation.preservation_verified().expect("verified");
  operation.preservation_finished(base.clone()).expect("prepared");
  operation.begin_sync(&base).expect("syncing");
  operation
    .plan_sync_result(checkout('f', Vec::new()))
    .expect("sync planned");
  operation.own_task(checkout('f', Vec::new()));
  operation.begin_check().expect("checking");
  operation
    .checked_result(CheckedResult {
      base: base.clone(),
      result: checkout('f', Vec::new()),
    })
    .expect("checked");
  assert_eq!(operation.phase(), LocalPhase::AwaitingApproval);
  assert!(operation.sync_result().is_some(), "the sync left its result recorded");
  let mut paused = operation.clone();
  paused.paused();
  assert!(paused.can_record_merged_outside(), "paused after the check");
  let mut operation = recorded(operation, "awaiting approval after a sync");
  assert_eq!(operation.phase(), LocalPhase::MergedAwaitingRestore);
  operation.begin_restore().expect("restoring");
  operation.restoration_verified().expect("restored");
  operation.begin_cleanup().expect("cleanup");
  operation.cleanup_finished().expect("removed");
  assert_eq!(operation.phase(), LocalPhase::Complete);
  assert!(!operation.reserves_checkout());
}
