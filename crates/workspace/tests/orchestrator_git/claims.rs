//! A write and a proposal's Apply respect Orchester's own claims (`checkout_reserved`): the local-integration lease
//! another holder has, a local integration that reserves the checkout, and a resolution recorded for a run's worktree.

use orchester_core::{
  blob::ContentHash,
  integration::local::{Checkout, LocalIntegrationStore, LocalOperation},
};
use orchester_exec::local_git::IntegrationLease;
use orchester_store::{ProjectDb, ResolutionStart};
use serde_json::json;

use super::{
  runs::{succeeded_run, working_run},
  scratch::{Scratch, refused},
};

/// What a local integration read of the checkout; what it holds does not matter here.
fn checkout(head: &str) -> Checkout {
  Checkout {
    root: "/project".to_owned(),
    git_dir: "/project/.git".to_owned(),
    branch: "main".to_owned(),
    head: head.to_owned(),
    index: ContentHash::new(&"a".repeat(64)).expect("hash"),
    files: Vec::new(),
    ignored: ContentHash::new(&"b".repeat(64)).expect("hash"),
    operation: None,
  }
}

#[test]
fn orchestrator_git_claims_a_held_lease_refuses_writes_and_applies() {
  let scratch = Scratch::new();
  scratch.write("README.md", "edited\n");
  let reset = scratch.proposed(&json!({ "operation": { "kind": "reset", "to": "HEAD", "mode": "hard" } }));
  let head = scratch.head("HEAD");
  let held = IntegrationLease::acquire(&scratch.root()).expect("local integration works in the checkout");
  let commit = scratch.call("git_commit", &json!({ "message": "Edit", "all": true }), true);
  assert_eq!(refused(commit), "checkout_reserved");
  let applied = scratch.apply(&reset).expect_err("refused");
  assert_eq!(super::scratch::code(&applied), "checkout_reserved");
  assert_eq!(scratch.read("README.md"), "edited\n", "nothing was discarded");
  drop(held);
  scratch.wrote("git_commit", &json!({ "message": "Edit", "all": true }));
  assert_ne!(scratch.head("HEAD"), head, "once the lease is free, the write runs");
}

#[test]
fn orchestrator_git_claims_a_reserved_checkout_refuses_writes() {
  let scratch = Scratch::new();
  let run = working_run(&scratch);
  let head = scratch.head("HEAD");
  let mut operation = LocalOperation::new(run.id(), checkout(&head)).expect("an operation");
  operation.prepared(checkout(&head)).expect("prepared");
  operation
    .begin_sync(&checkout(&head))
    .expect("it works in the checkout");
  assert!(operation.reserves_checkout());
  let mut db = ProjectDb::open(&scratch.root()).expect("the project's data");
  assert!(db.save_local_operation(&operation, None).expect("saved"));
  scratch.write("README.md", "edited\n");
  let commit = scratch.call("git_commit", &json!({ "message": "Edit", "all": true }), true);
  assert_eq!(refused(commit), "checkout_reserved");
  let tag = scratch.call("git_tag", &json!({ "name": "v1" }), true);
  assert_eq!(refused(tag), "checkout_reserved");
  assert_eq!(scratch.head("HEAD"), head);
  assert!(scratch.git(&["tag"]).is_empty());
}

#[test]
fn orchestrator_git_claims_a_recorded_resolution_refuses_writes_in_its_worktree() {
  let scratch = Scratch::new();
  let run = succeeded_run(&scratch);
  let mut db = ProjectDb::open(&scratch.root()).expect("the project's data");
  assert_eq!(
    db.begin_resolution(run.id(), orchester_core::time::Timestamp::now())
      .expect("recorded"),
    ResolutionStart::Begun
  );
  let named = run.number().get().to_string();
  let commit = scratch.call(
    "git_commit",
    &json!({ "run": named, "message": "Mine", "all": true }),
    true,
  );
  assert_eq!(refused(commit), "checkout_reserved");
  db.end_resolution(run.id()).expect("ended");
  scratch.wrote("git_commit", &json!({ "run": named, "message": "Mine", "all": true }));
}
