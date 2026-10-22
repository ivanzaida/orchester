//! Local integration of a run merged outside Orchester: a review that held nothing reads as complete beside the
//! *Merged* card, and one that holds the person's preserved originals keeps an action beside it that records the
//! merge and puts the originals back. Neither is a dead end.
use std::{fs, path::Path};

use orchester_core::{
  id::RunId,
  integration::local::{LocalIntegrationStore, LocalOperation, LocalPath, LocalPhase},
};
use orchester_store::ProjectDb;

use super::{
  domain_support::{Fixture, git},
  integration_support::Page,
};

fn record(page: &Page) -> Option<LocalOperation> {
  stored(page.root(), page.run())
}

fn stored(root: &Path, run: RunId) -> Option<LocalOperation> {
  ProjectDb::open(root)
    .expect("db")
    .local_operation(run)
    .expect("journal")
}

/// The *Merged* card, once, on the evidence side (`itt7z`, `pdT4W`).
fn merged_card(page: &mut Page) {
  page.until("the merged card", |window| {
    window
      .texts("local-external-integration")
      .is_some_and(|texts| texts.iter().any(|text| text == "Merged"))
  });
}

#[test]
fn local_integration_page_merged_outside_completes_a_paused_review_that_held_nothing() {
  let fixture = Fixture::new(false);
  let mut session = fixture.open();
  session.pause().expect("pause the review");
  drop(session);
  git(fixture.root(), &["merge", "--ff-only", "task"]);
  let mut page = Page::new(fixture);
  merged_card(&mut page);
  let (root, run) = (page.root().to_path_buf(), page.run());
  page.until("the record completes", |_| {
    stored(&root, run).is_some_and(|operation| operation.phase() == LocalPhase::Complete)
  });
  let settled = record(&page).expect("the record");
  assert_eq!(settled.phase(), LocalPhase::Complete);
  assert!(
    !settled.reserves_checkout(),
    "other tasks' local integration is not blocked"
  );
  assert!(
    !page.has("local-actions"),
    "nothing is left to do, and Merged is not said twice"
  );
  page.until("the record says the merge was made without Orchester", |window| {
    window
      .texts("local-recovery")
      .is_some_and(|texts| texts.iter().any(|text| text.starts_with("Without Orchester")))
  });
}

#[test]
fn local_integration_page_merged_outside_offers_recording_the_merge_for_preserved_originals() {
  let fixture = Fixture::new(false);
  fs::write(fixture.root().join("local.txt"), "base\nthe person's edit\n").expect("edit");
  let mut session = fixture.open();
  session.preserve("Before the merge").expect("preserve");
  drop(session);
  git(fixture.root(), &["merge", "--ff-only", "task"]);
  let head = git(fixture.root(), &["rev-parse", "HEAD"]);
  let mut page = Page::new(fixture);
  merged_card(&mut page);
  page.ready("local-record-outside");
  assert!(page.has("local-restore-next"), "restoring the originals comes next");
  page.click("local-record-outside");
  page.ready("local-restore");
  let recorded = record(&page).expect("the record");
  assert_eq!(recorded.phase(), LocalPhase::MergedAwaitingRestore);
  assert_eq!(recorded.merged_outside(), Some(head.as_str()));
  page.click("local-restore");
  page.ready("local-review-cleanup");
  assert_eq!(
    fs::read_to_string(page.root().join("local.txt")).expect("the person's file"),
    "base\nthe person's edit\n"
  );
  assert_eq!(
    git(page.root(), &["rev-parse", "HEAD"]),
    head,
    "nothing was merged again"
  );
}

/// The tab says merged by the Summary's rule: the run's delivery target holds the work, not the branch checked out.
#[test]
fn local_integration_page_merged_outside_names_the_delivery_target_not_the_checked_out_branch() {
  let fixture = Fixture::new(false);
  git(fixture.root(), &["checkout", "-b", "feature"]);
  git(fixture.root(), &["merge", "--no-ff", "--no-edit", "task"]);
  let mut page = Page::new(fixture);
  page.ready("local-phase");
  assert!(
    !page.has("local-external-integration"),
    "feature holds the work, but the run delivers into main: the Summary still shows Ready"
  );
  git(page.root(), &["branch", "-f", "main", "feature"]);
  merged_card(&mut page);
  let said = page.window.texts("local-external-integration").expect("card").join(" ");
  assert!(said.contains("is in main"), "{said}");
  assert!(!page.has("local-phase"), "the card stands for what the page is about");
}

#[test]
fn local_integration_page_part_merged_outside_reconciles_a_held_commit_then_cancels() {
  let fixture = Fixture::new(false);
  fs::write(
    fixture.root().join("local.txt"),
    "base
the person's commit
",
  )
  .expect("edit");
  git(fixture.root(), &["add", "local.txt"]);
  let mut session = fixture.open();
  session.refresh().expect("review the edit");
  let path = LocalPath::new("local.txt").expect("path");
  let tree = session.selected_commit_tree(std::slice::from_ref(&path)).expect("tree");
  session.commit(&[path], &tree, "the person's commit").expect("commit");
  drop(session);
  let first = git(&fixture.task_root, &["rev-parse", "HEAD"]);
  fs::write(
    fixture.task_root.join("remaining.txt"),
    "not delivered
",
  )
  .expect("edit");
  git(&fixture.task_root, &["add", "remaining.txt"]);
  git(&fixture.task_root, &["commit", "-m", "second result"]);
  git(fixture.root(), &["merge", "--no-edit", &first]);
  let mut page = Page::new(fixture);
  page.ready("local-external-integration");
  assert!(
    !page.has("local-record-outside"),
    "part of the work is no merge to record"
  );
  page.ready("local-reconcile");
  page.click("local-reconcile");
  page.ready("local-finish-cancel");
  page.click("local-finish-cancel");
  page.click("local-confirm-scope");
  page.click("local-confirm");
  let (root, run) = (page.root().to_path_buf(), page.run());
  page.until("the integration is cancelled", |_| {
    stored(&root, run).is_some_and(|operation| operation.phase() == LocalPhase::Canceled)
  });
  assert!(!record(&page).expect("the record").reserves_checkout());
}
