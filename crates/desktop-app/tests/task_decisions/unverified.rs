//! The decision for a step that waits unverified (what it could not check, *Accept … as it is* first) or was not
//! run again after a failed verdict (the verdict shown, *Run … again* first), on the `decisions` scenario.

use super::{OFFLINE, Page};

#[test]
fn decision_unverified_lists_what_was_not_checked_and_offers_accept_run_again_and_return() {
  let mut page = Page::seeded("decisions");
  page.open("waits unverified after Review", "task-retry-decision");
  assert_eq!(
    page.text("task-notice-title"),
    "Unverified: Review could not check 2 things"
  );
  assert!(page.text("task-notice").contains("no retry was used"));
  assert_eq!(page.text("task-unverified-head"), "What Review could not check");
  assert!(
    page
      .text("task-unverified-0")
      .contains("Tasks · a filter that matches nothing")
  );
  assert!(
    page
      .text("task-unverified-1")
      .contains("could not be started on a damaged run row")
  );
  assert_eq!(page.text("task-accept-step"), "Accept Review as it is");
  assert_eq!(page.text("task-allow-retry"), "Run Review again");
  assert!(page.has("task-return-with-note"));
  // Each decision goes to the runner, which is not connected here: they wait, and say why.
  for button in ["task-accept-step", "task-allow-retry", "task-stop-run"] {
    assert!(page.locked(button), "{button}");
  }
  assert_eq!(page.text("task-retry-offline"), OFFLINE);
}

#[test]
fn decision_unchanged_after_a_failed_verdict_shows_it_and_runs_again_first() {
  let mut page = Page::seeded("decisions");
  page.open("waits unchanged after Review failed", "task-retry-decision");
  assert_eq!(page.text("task-notice-title"), "Review was not run again");
  // The verdict it was not run again for is what Review reported.
  assert!(page.text("task-retry-report-text").contains("UTC day"));
  assert!(!page.has("task-unverified"));
  assert_eq!(page.text("task-allow-retry"), "Run Review again");
  assert_eq!(page.text("task-accept-step"), "Accept Review as it is");
  let (retry, accept) = (page.bounds("task-allow-retry").y, page.bounds("task-accept-step").y);
  assert!(
    retry < accept,
    "Run again comes first, accepting second: {retry} < {accept}"
  );
}
