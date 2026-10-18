//! Local integration of a run merged outside Orchester: a review that held nothing reads as complete beside the
//! *Merged* card, and one that holds the person's preserved originals keeps an action beside it that records the
//! merge and puts the originals back. Neither is a dead end.
use std::{fs, path::Path};

use orchester_core::{
  id::RunId,
  integration::local::{LocalIntegrationStore, LocalOperation, LocalPhase},
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

/// The *Merged* card, alone in the inspector (`local-actions`) or above the step's (`local-external`).
fn merged_card(page: &mut Page, id: &str) {
  let title = format!("{id}-title");
  page.until("the merged card", |window| {
    window
      .texts(&title)
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
  merged_card(&mut page, "local-actions");
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
  assert!(!page.has("local-reconcile"), "nothing is left to reconcile");
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
  merged_card(&mut page, "local-external");
  page.ready("local-record-outside");
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
