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
