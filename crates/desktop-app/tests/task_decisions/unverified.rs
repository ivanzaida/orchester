//! The decision for a step that waits unverified (what it could not check, *Accept … as it is* first) or was not
//! run again after a failed verdict (the verdict shown, *Run … again* first), on the `decisions` scenario.

use super::{OFFLINE, Page};

#[test]
fn decision_unverified_lists_what_was_not_checked_and_offers_accept_run_again_and_return() {
  let mut page = Page::seeded("decisions");
  page.open("waits unverified after Review", "task-retry-decision");
  let event = page.text("task-event-text");
  assert!(
    event.starts_with("Unverified: Review could not check 2 things"),
    "{event}"
  );
  assert!(event.contains("no retry was used"), "{event}");
  // The header says why it waits, not that retries were used.
  let status = page.text("task-status");
  assert!(status.contains("could not check everything"), "{status}");
  assert!(!status.contains("retries used"), "{status}");
  // Running Review again is a choice of its own: returning the work starts at the step Review sends it back to.
  let options = page.options("task-return-step");
  assert!(options.contains(&"03 Implement · default".to_owned()), "{options:?}");
  assert!(page.text("task-return-note-field-label").contains("Note for Builder"));
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
  let event = page.text("task-event-text");
  assert!(event.starts_with("Review was not run again"), "{event}");
  assert!(event.contains("last failed on this same revision"), "{event}");
  assert!(page.text("task-status").contains("not run again"));
  // Review sent the work back to Implement, which changed nothing: the work goes back there by default.
  let options = page.options("task-return-step");
  assert!(options.contains(&"03 Implement · default".to_owned()), "{options:?}");
  assert!(page.text("task-return-note-field-label").contains("Note for Builder"));
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
