use orchester_core::{
  blob::ContentHash,
  id::RunId,
  integration::local::{CheckedResult, Checkout, LocalError, LocalOperation, LocalPath, LocalPhase},
};

fn checkout(head: char) -> Checkout {
  Checkout {
    root: String::from("/project"),
    git_dir: String::from("/project/.git"),
    branch: String::from("main"),
    head: head.to_string().repeat(40),
    index: ContentHash::new(&"a".repeat(64)).expect("hash"),
    files: Vec::new(),
    ignored: ContentHash::new(&"b".repeat(64)).expect("hash"),
    operation: None,
  }
}

#[test]
fn local_integration_separates_edit_check_approval_and_completed_merge() {
  let base = checkout('a');
  let mut operation = LocalOperation::new(RunId::generate(), base.clone()).expect("operation");
  operation.prepared(base.clone()).expect("prepared");
  operation.begin_sync(&base).expect("sync");
  operation.begin_check().expect("check");
  let result = CheckedResult {
    base,
    result: checkout('b'),
  };
  operation.checked_result(result.clone()).expect("passed");
  assert_eq!(operation.begin_merge(&result), Err(LocalError::ApprovalRequired));
  let mut stale = result.clone();
  stale.result.head = "c".repeat(40);
  assert_eq!(operation.approve(&stale), Err(LocalError::Stale));
  operation.approve(&result).expect("person approved exact result");
  operation.begin_merge(&result).expect("merge intent");
  operation.record_merge("b".repeat(40)).expect("completed merge");
  assert_eq!(operation.phase(), LocalPhase::Complete);
  assert_eq!(operation.begin_merge(&result), Err(LocalError::WrongPhase));
  operation.paused();
  operation.resume_saved_phase();
  assert_eq!(operation.phase(), LocalPhase::Complete);
}

#[test]
fn local_integration_checkout_preflight_refuses_foreign_operation_before_stale_comparison() {
  let reviewed = checkout('a');
  let mut current = checkout('b');
  current.operation = Some(String::from("rebase-merge:"));
  assert_eq!(reviewed.ensure_current(&current), Err(LocalError::ForeignOperation));
  current.operation = None;
  assert_eq!(reviewed.ensure_current(&current), Err(LocalError::Stale));
}

#[test]
fn local_integration_portable_paths_refuse_metadata_traversal_and_windows_aliases() {
  for path in [
    "../file",
    ".git/config",
    "dir/.GIT/index",
    "a\\b",
    "C:file",
    "a//b",
    "CON",
    "dir/NUL.txt",
    "dir/file.",
    "LPT1.txt",
  ] {
    assert!(LocalPath::new(path).is_err(), "refused {path}");
  }
  assert!(LocalPath::new("src/deep/file name.rs").is_ok());
}

#[test]
fn local_integration_preservation_removal_requires_completed_verified_restoration() {
  let mut operation = LocalOperation::new(RunId::generate(), checkout('a')).expect("operation");
  assert_eq!(operation.begin_cleanup(), Err(LocalError::NotVerified));
  assert_eq!(operation.begin_restore(), Err(LocalError::WrongPhase));
  assert_eq!(
    operation.begin_commit(&checkout('b'), "message"),
    Err(LocalError::Stale)
  );
}

#[test]
fn local_integration_refresh_cannot_erase_pending_merge_intent_after_interruption() {
  let base = checkout('a');
  let mut operation = LocalOperation::new(RunId::generate(), base.clone()).expect("operation");
  operation.prepared(base.clone()).expect("prepare");
  operation.begin_sync(&base).expect("sync");
  operation.begin_check().expect("check");
  let exact = CheckedResult {
    base,
    result: checkout('b'),
  };
  operation.checked_result(exact.clone()).expect("checked");
  operation.approve(&exact).expect("approve");
  operation.begin_merge(&exact).expect("intent");
  operation.needs_reconciliation();
  assert!(operation.merge_pending());
  assert_eq!(operation.refresh(checkout('b')), Err(LocalError::WrongPhase));
  operation.resume_saved_phase();
  assert_eq!(operation.phase(), LocalPhase::Merging);
  assert_eq!(operation.refresh(checkout('b')), Err(LocalError::WrongPhase));
  operation
    .record_merge(exact.result.head)
    .expect("reconciled completion");
  assert!(!operation.merge_pending());
}

#[test]
fn local_integration_only_read_only_reviews_release_checkout_reservation() {
  let base = checkout('a');
  let mut review = LocalOperation::new(RunId::generate(), base.clone()).expect("review");
  assert!(!review.reserves_checkout());
  review.prepared(base.clone()).expect("read-only preparation");
  assert!(!review.reserves_checkout());
  review.begin_sync(&base).expect("pending mutation");
  assert!(review.reserves_checkout());
  review.own_task(base.clone());
  review.refresh(base.clone()).expect("review again");
  assert!(
    review.reserves_checkout(),
    "owned task recovery cannot be released by refresh"
  );

  let mut commit = LocalOperation::new(RunId::generate(), base.clone()).expect("review");
  commit.begin_commit(&base, "selected commit").expect("intent");
  assert!(commit.reserves_checkout());
  commit
    .commit_intent(vec![LocalPath::new("local.txt").expect("path")], "b".repeat(40))
    .expect("selection");
  commit.commit_finished(base).expect("commit completed");
  assert!(
    commit.reserves_checkout(),
    "retained commit originals still reserve the checkout"
  );
}

#[test]
fn local_integration_completed_commit_release_retains_audit_and_requires_current_checkout() {
  let base = checkout('a');
  let mut operation = LocalOperation::new(RunId::generate(), base.clone()).expect("review");
  operation.begin_commit(&base, "selected").expect("intent");
  operation.commit_intent(Vec::new(), "b".repeat(40)).expect("tree");
  assert!(!operation.can_cancel(), "pending mutation must reconcile");
  assert_eq!(operation.begin_cancel(&base), Err(LocalError::WrongPhase));
  let committed = checkout('b');
  operation.commit_finished(committed.clone()).expect("completed");
  assert!(operation.can_cancel());
  assert_eq!(operation.begin_cancel(&base), Err(LocalError::Stale));
  operation.begin_cancel(&committed).expect("release intent");
  operation.cancellation_verified().expect("release");
  assert!(!operation.reserves_checkout());
  assert_eq!(operation.commit_tree(), Some("b".repeat(40).as_str()));
  assert_eq!(operation.reviewed(), &committed);
}

#[test]
fn local_integration_checked_release_invalidates_approval_and_cannot_cancel_pending_merge() {
  let base = checkout('a');
  let mut operation = LocalOperation::new(RunId::generate(), base.clone()).expect("review");
  operation.prepared(base.clone()).expect("prepare");
  operation.begin_sync(&base).expect("sync");
  operation.own_task(checkout('b'));
  operation.begin_check().expect("check");
  let exact = CheckedResult {
    base: base.clone(),
    result: checkout('b'),
  };
  operation.checked_result(exact.clone()).expect("passed");
  operation.approve(&exact).expect("private domain decision");
  let mut pending = operation.clone();
  pending.begin_merge(&exact).expect("merge intent");
  assert!(!pending.can_cancel());
  assert_eq!(pending.begin_cancel(&base), Err(LocalError::WrongPhase));
  operation.begin_cancel(&base).expect("cancel checked preparation");
  operation.cancellation_verified().expect("release");
  assert!(operation.checked().is_none());
  assert_eq!(operation.begin_merge(&exact), Err(LocalError::Stale));
  assert!(!operation.reserves_checkout());
}

#[test]
fn local_integration_abort_intent_survives_serialization_and_only_finishes_at_the_exact_result() {
  let base = checkout('a');
  let task = checkout('b');
  let mut conflicts = task.clone();
  conflicts.operation = Some("MERGE_HEAD:owned".into());
  // Resolving .gitignore can expose a retained file; abort restores HEAD's classification.
  conflicts.ignored = ContentHash::new(&"c".repeat(64)).expect("resolved ignore identity");
  let mut operation = LocalOperation::new(RunId::generate(), base.clone()).expect("operation");
  operation.prepared(base.clone()).expect("prepared");
  operation.begin_sync(&base).expect("sync");
  operation.own_task(conflicts.clone());
  operation.task_conflicts().expect("conflicts");
  operation.plan_task_abort(task.clone()).expect("abort intent");
  let encoded = serde_json::to_vec(&operation).expect("journal");
  let mut recovered: LocalOperation = serde_json::from_slice(&encoded).expect("restart");
  assert_eq!(recovered.phase(), LocalPhase::NeedsReconciliation);
  assert_eq!(recovered.task_abort_result(), Some(&task));
  assert!(recovered.reserves_checkout());
  assert_eq!(recovered.refresh(base.clone()), Err(LocalError::WrongPhase));
  recovered.paused();
  recovered.resume_saved_phase();
  assert_eq!(recovered.phase(), LocalPhase::NeedsReconciliation);
  assert_eq!(recovered.ensure_task_owned(&conflicts), Err(LocalError::Reconcile));
  assert_eq!(recovered.plan_task_commit(), Err(LocalError::WrongPhase));
  assert_eq!(recovered.begin_check(), Err(LocalError::WrongPhase));
  let mut stale_classification = task.clone();
  stale_classification.ignored.clone_from(&conflicts.ignored);
  assert_eq!(
    recovered.task_abort_finished(base.clone(), stale_classification),
    Err(LocalError::Reconcile)
  );
  assert_eq!(
    recovered.task_abort_finished(base.clone(), checkout('c')),
    Err(LocalError::Reconcile)
  );
  assert_eq!(recovered.task_abort_result(), Some(&task));
  recovered
    .task_abort_finished(base, task.clone())
    .expect("exact completion");
  assert_eq!(recovered.phase(), LocalPhase::Paused);
  assert_eq!(recovered.owned_task(), Some(&task));
  assert!(recovered.task_abort_result().is_none());
  assert!(recovered.checked().is_none());
  assert!(recovered.merged().is_none());
}

#[test]
fn local_integration_unchanged_sync_intent_releases_only_exact_clean_checkouts() {
  let base = checkout('a');
  let task = checkout('b');
  let mut operation = LocalOperation::new(RunId::generate(), base.clone()).expect("operation");
  operation.prepared(base.clone()).expect("prepare");
  operation.begin_sync(&base).expect("sync intent");
  operation.own_task(task.clone());
  let mut predicted = task.clone();
  predicted.operation = Some("MERGE_HEAD:owned".into());
  operation.plan_sync_result(predicted).expect("prediction");
  assert!(!operation.can_cancel());
  assert_eq!(
    operation.sync_not_started(&checkout('c'), &task),
    Err(LocalError::Stale)
  );
  assert_eq!(
    operation.sync_not_started(&base, &checkout('c')),
    Err(LocalError::ForeignOperation)
  );
  let mut foreign = task.clone();
  foreign.operation = Some("MERGE_HEAD:foreign".into());
  assert_eq!(
    operation.sync_not_started(&base, &foreign),
    Err(LocalError::ForeignOperation)
  );
  assert_eq!(operation.phase(), LocalPhase::Syncing);
  let encoded = serde_json::to_vec(&operation).expect("journal");
  let mut recovered: LocalOperation = serde_json::from_slice(&encoded).expect("restart");
  recovered.resume_saved_phase();
  recovered.sync_not_started(&base, &task).expect("safe release");
  assert_eq!(recovered.phase(), LocalPhase::Prepared);
  assert!(recovered.sync_result().is_none());
  assert!(recovered.checked().is_none());
  assert!(recovered.merged().is_none());
  assert!(recovered.can_cancel());
  recovered.begin_cancel(&base).expect("cancel");
  recovered.cancellation_verified().expect("verified release");
  assert!(!recovered.reserves_checkout());
}
