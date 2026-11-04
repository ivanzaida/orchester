//! What tries a run's merge again from its attempt page: *Try again* on a run the merge queue stopped puts it back in
//! its place there, and once the queue waits for approval the page sends the person to the queue instead; Orchester's
//! own retry counts down until *Stop retrying*, or until *Try now* tries at once, with the lease another operation
//! holds or QA's `hold_integration` holding it.

use std::time::{Duration, Instant};

use orchester_app::{
  mcp::tools::{HeldLease, hold_integration},
  routes::{ProjectPage, Route},
};
use orchester_core::{merge_queue::QueueState, time::Timestamp};
use orchester_exec::local_git::IntegrationLease;
use orchester_store::ProjectDb;
use serde_json::json;

use super::{GIT_WORK, Page};

/// How long Orchester waits before its first retry, which *Try now* cuts short.
const RETRY_WAIT: Duration = Duration::from_secs(10);

#[test]
fn merge_attempt_page_try_again_puts_a_run_the_queue_stopped_back_in_its_place() {
  let mut page = Page::new();
  let route = page.open("merges-not-merged", "Per-endpoint rate limits");
  let Route::Project(_, ProjectPage::MergeAttempt(_, number)) = route else {
    panic!("the attempt page");
  };
  assert_eq!(
    page.window.text("merge-attempt-status-text"),
    "Stopped in the merge queue"
  );
  assert_eq!(
    page.window.text("merge-attempt-1-via"),
    format!("delivering run #{}", number.get())
  );
  page.window.press("merge-attempt-try-again");
  let repo = page.repo.clone();
  page.window.until_within(GIT_WORK, "the run back in the queue", |_| {
    let db = ProjectDb::open(&repo).expect("the project database");
    db.merge_queue()
      .expect("the queue")
      .iter()
      .any(|entry| entry.state.in_queue())
  });
}

/// A run the queue took back and checked again waits for approval there: its page says so and opens the queue, and
/// offers no *Try again* that would merge it from its task page past the approval; Merges lists it as not merged no
/// more.
#[test]
fn merge_attempt_page_a_run_back_in_the_queue_awaiting_approval_opens_the_queue() {
  let mut page = Page::new();
  let route = page.open("merges-not-merged", "Per-endpoint rate limits");
  page.window.press("merge-attempt-try-again");
  let repo = page.repo.clone();
  // The queue may move the entry on between the read and the compare-and-swap, which then refuses it: read it again
  // and swap until the swap lands.
  page
    .window
    .until_within(GIT_WORK, "the queue checked the run again", |_| {
      let mut db = ProjectDb::open(&repo).expect("the project database");
      let Some(mut entry) = db
        .merge_queue()
        .expect("the queue")
        .into_iter()
        .find(|entry| entry.state.in_queue())
      else {
        return false;
      };
      let before = entry.state.clone();
      entry.state = QueueState::WaitingForApproval;
      db.save_merge_queue_entry(&entry, &before, (&[], Timestamp::now()))
        .expect("saved")
    });
  page
    .window
    .navigation()
    .go(Route::Project(page.project, ProjectPage::Merges));
  // Merges shows the rows it last read at once, the run among them, and drops it once it reads the queue again.
  page
    .window
    .until_within(GIT_WORK, "the merges without the run awaiting approval", |window| {
      window.texts("merges-not-merged-rows").is_some_and(|rows| {
        let rows = rows.join(" ");
        rows.contains("Add retry to webhook sender") && !rows.contains("Per-endpoint rate limits")
      })
    });
  page.window.navigation().go(route);
  page.until_read();
  assert_eq!(
    page.window.text("merge-attempt-status-text"),
    "Waiting for your approval in the merge queue"
  );
  assert_eq!(page.window.text("merge-attempt-open-queue"), "Open merge queue");
  assert!(!page.shows("merge-attempt-try-again"));
}

#[test]
fn merge_attempt_page_retrying_counts_down_until_stop_retrying() {
  let mut page = Page::new();
  page.open("merges-not-merged", "Add retry to webhook sender");
  let held = IntegrationLease::acquire(&page.repo).expect("another operation's lease");
  page.window.press("merge-attempt-try-again");
  page.until_status("Retrying by itself");
  let status = page.window.text("merge-attempt-status-text");
  assert!(status.starts_with("Retrying by itself · attempt 1 of 3"), "{status}");
  assert_eq!(page.window.text("merge-attempt-try-now"), "Try now");
  assert_eq!(page.window.text("merge-attempt-stop-retrying"), "Stop retrying");
  assert!(
    page
      .window
      .text("merge-attempt-status-meta")
      .contains("nothing merged so far")
  );
  page.window.press("merge-attempt-stop-retrying");
  page.until_status("Not merged into main");
  drop(held);
  assert_eq!(page.window.text("merge-attempt-try-again"), "Try again");
  assert_eq!(page.window.text("merge-attempt-attempts-head-count"), "3");
}

/// QA's `hold_integration` holds the lease the way another operation does: *Try again* is refused and retried by
/// Orchester; once the lease is let go, *Try now* tries at once, before the 10 s wait, and gets past the lock to the
/// README *Payload size guard* left changed, which a lasting cause ends Not merged.
#[test]
fn merge_attempt_page_hold_integration_retries_until_let_go_then_try_now_tries_at_once() {
  let mut page = Page::new();
  page.open("merges-not-merged", "Add retry to webhook sender");
  let (link, lease) = (page.window.link.clone(), HeldLease::default());
  let held = hold_integration(&link, &lease, &json!({ "held": true })).expect("the lease");
  assert_eq!(held["held"], true, "{held}");
  page.window.press("merge-attempt-try-again");
  page.until_status("Retrying by itself · attempt 1 of 3 in");
  let free = hold_integration(&link, &lease, &json!({ "held": false })).expect("let go");
  assert_eq!(free, json!({ "held": false, "project": null }));
  let pressed = Instant::now();
  page.window.press("merge-attempt-try-now");
  page.until_status("Not merged into main");
  assert!(pressed.elapsed() < RETRY_WAIT, "tried before the wait ended");
  let meta = page.window.text("merge-attempt-status-meta");
  assert!(meta.contains("by Orchester, retry 1 of 3"), "{meta}");
  let checks = page.window.text("merge-attempt-checks");
  assert!(
    checks.contains("Integration lock · Scratch folder"),
    "past the lock: {checks}"
  );
  assert!(checks.contains("left changed"), "{checks}");
}
