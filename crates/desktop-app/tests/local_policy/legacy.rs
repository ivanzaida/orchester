use std::fs;

use orchester_core::{
  integration::local::LocalIntegrationStore,
  store::{MergeStore, RunStore},
};
use orchester_store::ProjectDb;
use serde_json::json;

use super::{
  client::{Client, ok, value},
  domain_support::{Fixture, git},
  integration_support::Page,
};

fn arguments(page: &Page) -> serde_json::Value {
  json!({"project": ProjectDb::open(page.root()).expect("db").project().id().to_string(), "run":page.run().to_string()})
}
#[test]
fn local_policy_authenticated_legacy_domain_merge_keeps_original_gates_and_actual_outcome() {
  let mut page = Page::new(Fixture::new(false));
  page.ready("local-review-preserve");
  let before = ProjectDb::open(page.root())
    .expect("db")
    .run(page.run())
    .expect("run")
    .expect("exists");
  let source = git(&page.fixture.task_root, &["rev-parse", "HEAD"]);
  let client = Client::new(&mut page, false);
  let args = arguments(&page);
  let result = client.call(&mut page, "merge_run", &args);
  ok(&result);
  assert_eq!(value(&result)["merge"]["kind"], "fast_forward");
  assert_eq!(git(page.root(), &["rev-parse", "HEAD"]), source);
  let db = ProjectDb::open(page.root()).expect("db");
  assert_eq!(
    db.run_merge(page.run())
      .expect("record")
      .expect("actual outcome")
      .revision
      .as_str(),
    source
  );
  assert!(db.local_operation(page.run()).expect("no managed receipt").is_none());
  let after = db.run(page.run()).expect("run").expect("exists");
  assert_eq!(before.status(), after.status());
  assert_eq!(before.snapshot(), after.snapshot());
}

/// ORC-60: the run's branch fast-forwarded onto newer main before it committed anything, and main moved on since.
/// Main's commits are not the run's result: `merge_run` merges it, and no reconciliation is asked for.
#[test]
fn local_policy_legacy_merge_of_a_run_fast_forwarded_onto_newer_main_is_not_scoped() {
  let commit = |root: &std::path::Path, file: &str, message: &str| {
    fs::write(root.join(file), format!("{message}\n")).expect("write");
    git(root, &["add", file]);
    git(root, &["commit", "-m", message]);
    git(root, &["rev-parse", "HEAD"])
  };
  let fixture = Fixture::worked(false, ("moved-ff-", "task"), |root, task| {
    commit(root, "landed.txt", "landed work");
    git(task, &["merge", "main"]);
    vec![commit(task, "task.txt", "task work")]
  });
  commit(fixture.root(), "later.txt", "later main work");
  let mut page = Page::new(fixture);
  page.ready("local-review-preserve");
  let source = git(&page.fixture.task_root, &["rev-parse", "HEAD"]);
  let client = Client::new(&mut page, false);
  let args = arguments(&page);
  let result = client.call(&mut page, "merge_run", &args);
  ok(&result);
  assert_eq!(value(&result)["merge"]["kind"], "merge_commit");
  assert_eq!(
    git(page.root(), &["merge-base", "--is-ancestor", &source, "HEAD"]),
    "",
    "main holds the run's work"
  );
  assert_eq!(git(page.root(), &["show", "HEAD:later.txt"]), "later main work");
}

#[test]
fn local_policy_legacy_domain_external_complete_partial_and_scoped_results_make_no_extra_commit() {
  for scope in ["complete", "partial", "scoped"] {
    let fixture = Fixture::new(false);
    let first = git(&fixture.task_root, &["rev-parse", "HEAD"]);
    if scope == "partial" {
      fs::write(fixture.task_root.join("remaining.txt"), "not yet integrated\n").expect("task");
      git(&fixture.task_root, &["add", "remaining.txt"]);
      git(&fixture.task_root, &["commit", "-m", "remaining result"]);
    }
    if scope == "scoped" {
      fs::write(fixture.root().join("unrelated.txt"), "unrelated\n").expect("main");
      git(fixture.root(), &["add", "unrelated.txt"]);
      git(fixture.root(), &["commit", "-m", "unrelated main"]);
      git(fixture.root(), &["cherry-pick", &first]);
    } else {
      git(fixture.root(), &["merge", "--ff-only", &first]);
    }
    let mut page = Page::new(fixture);
    page.ready("local-external-integration");
    let notice = page
      .window
      .texts("local-external-integration")
      .expect("notice")
      .join(" ");
    // The evidence card says it once; with no record holding anything there is no step to take (`itt7z`).
    assert!(!page.has("local-actions"), "the inspector repeats nothing");
    if scope == "complete" {
      // The tab says what the Summary says of a complete merge done outside Orchester.
      assert!(
        notice.contains("Merged") && notice.contains("it was merged without Orchester"),
        "{notice}"
      );
      let trail = page.window.texts("local-phase-trail").expect("trail").join(" ");
      assert!(trail.ends_with("Merged"), "{trail}");
      assert!(!notice.contains("reconciliation"));
    } else {
      assert!(notice.contains("Integration result needs reconciliation"), "{notice}");
    }
    let before = git(page.root(), &["rev-parse", "HEAD"]);
    let client = Client::new(&mut page, false);
    let args = arguments(&page);
    let result = client.call(&mut page, "merge_run", &args);
    assert!(result["isError"].as_bool().unwrap_or(false));
    assert_eq!(
      value(&result)["code"],
      if scope == "complete" {
        "already_merged"
      } else {
        "scoped_integration_required"
      }
    );
    assert_eq!(git(page.root(), &["rev-parse", "HEAD"]), before);
    let db = ProjectDb::open(page.root()).expect("db");
    assert!(db.run_merge(page.run()).expect("no forged history").is_none());
    assert!(db.local_operation(page.run()).expect("no approval").is_none());
  }
}

#[test]
fn local_policy_managed_domain_cannot_mint_review_receipt_or_fall_through_legacy() {
  let fixture = Fixture::new(false);
  let mut session = fixture.open();
  session.prepare_clean().expect("prepare");
  session.sync().expect("actual Check");
  let exact = session.operation().checked().expect("checked").clone();
  let generation = session.operation().version();
  drop(session);
  let mut page = Page::new(fixture);
  page.ready("local-confirm");
  let workspace = page.window.launch.workspace().expect("workspace");
  workspace
    .set_automation_changes_permissions(true)
    .expect("private grant");
  workspace.set_automation_answers_approvals(true).expect("private grant");
  let client = Client::new(&mut page, false);
  let args = arguments(&page);
  let result = client.call(&mut page, "merge_run", &args);
  assert_eq!(value(&result)["code"], "needs_decision");
  assert_eq!(git(page.root(), &["rev-parse", "HEAD"]), exact.base.head);
  let db = ProjectDb::open(page.root()).expect("db");
  let operation = db.local_operation(page.run()).expect("op").expect("exists");
  assert_eq!(operation.version(), generation);
  assert_eq!(operation.checked(), Some(&exact));
  assert!(!operation.merge_pending());
  assert!(operation.merged().is_none());
  assert!(db.run_merge(page.run()).expect("no forged merge").is_none());
}

#[test]
fn local_policy_mounted_preparation_review_blocks_legacy_even_before_durable_operation() {
  let mut page = Page::new(Fixture::new(false));
  page.ready("local-review-preserve");
  page
    .window
    .tree
    .get_element_by_id_mut("local-review-preserve")
    .expect("mode button")
    .click();
  let project = ProjectDb::open(page.root()).expect("db").project().id();
  assert!(
    page
      .window
      .launch
      .services()
      .expect("session")
      .jobs
      .of(project)
      .state()
      .local_decision_pending(),
    "the decision is reserved before any render or queued legacy work"
  );
  page.ready("local-preservation-name");
  assert!(
    ProjectDb::open(page.root())
      .expect("db")
      .local_operation(page.run())
      .expect("op")
      .is_none()
  );
  let client = Client::new(&mut page, false);
  let args = arguments(&page);
  let before = git(page.root(), &["rev-parse", "HEAD"]);
  let result = client.call(&mut page, "merge_run", &args);
  assert_eq!(value(&result)["code"], "needs_decision");
  assert_eq!(git(page.root(), &["rev-parse", "HEAD"]), before);
}
#[test]
fn local_policy_legacy_ignored_collision_preserves_original_bytes_and_does_not_merge() {
  let fixture = Fixture::new(false);
  fs::write(fixture.task_root.join("ignored.txt"), "task committed bytes\n").expect("task");
  git(&fixture.task_root, &["add", "-f", "ignored.txt"]);
  git(&fixture.task_root, &["commit", "-m", "task ignored path"]);
  fs::write(fixture.root().join("ignored.txt"), "personal ignored original\n").expect("original");
  let mut page = Page::new(fixture);
  page.ready("local-review-preserve");
  let before = git(page.root(), &["rev-parse", "HEAD"]);
  let client = Client::new(&mut page, false);
  let args = arguments(&page);
  let result = client.call(&mut page, "merge_run", &args);
  assert_eq!(value(&result)["code"], "merge_failed");
  assert_eq!(git(page.root(), &["rev-parse", "HEAD"]), before);
  assert_eq!(
    fs::read(page.root().join("ignored.txt")).expect("original"),
    b"personal ignored original\n"
  );
  assert!(
    ProjectDb::open(page.root())
      .expect("db")
      .run_merge(page.run())
      .expect("no receipt")
      .is_none()
  );
}
