//! A pull request's page: its header, tabs and toolbar, the changed files beside the selected file's diff, and
//! what stands in their place (`c07NAC`): no changes, too large, a diff Orchester cannot read, and GitHub not
//! answering with the diff read before kept under a notice.

use orchester_app::routes::{ProjectPage, Route};

use crate::harness::Seeded;

impl Seeded {
  /// Pull request `number`'s page, once its read ended.
  fn pull_request(&mut self, number: u64) {
    self
      .window
      .navigation()
      .go(Route::Project(self.project, ProjectPage::PullRequest(number)));
    self.read_ended();
  }

  fn read_ended(&mut self) {
    self.window.until("the read to end", |window| {
      window
        .texts("pull-request-header")
        .is_some_and(|texts| !texts.iter().any(|text| text.contains("Syncing…")))
    });
  }
}

#[test]
fn pull_request_page_shows_its_files_beside_the_selected_files_diff() {
  let mut seeded = Seeded::answering("ok");
  seeded.pull_request(142);
  let key = seeded.key("Add retry to webhook sender");
  assert_eq!(seeded.text("pull-request-title"), "Add retry to webhook sender");
  let organization = seeded.text("pull-request-organization");
  for part in [
    "Open",
    "Review required",
    "main",
    key.as_str(),
    "5 passed",
    "1 running",
    "Synced just now",
  ] {
    assert!(organization.contains(part), "{part} in {organization}");
  }
  assert!(organization.contains("Head "), "{organization}");
  assert_eq!(seeded.text("pull-request-tab-files"), "Files changed 6");
  assert_eq!(
    seeded.text("pull-request-tabs-reason"),
    "Agent feedback, Agent checks and Overview are planned."
  );
  assert_eq!(seeded.text("pull-request-total"), "6 files · +133 / −15");
  assert!(seeded.shows("changes-file:src/webhooks/sender.rs"));
  assert_eq!(
    seeded.text("changes-diff-path"),
    "migrations/0042_webhook_delivery_attempts.sql"
  );
  seeded.window.press("changes-file:src/webhooks/sender.rs");
  assert_eq!(seeded.text("changes-diff-path"), "src/webhooks/sender.rs");
  seeded.window.set("pull-request-find-file", "tests/");
  assert!(!seeded.shows("changes-file:src/webhooks/sender.rs"));
  assert!(seeded.shows("changes-file:tests/webhooks/jitter.rs"));
}

#[test]
fn pull_request_page_says_what_stands_in_place_of_the_diff() {
  let mut seeded = Seeded::answering("ok");
  seeded.pull_request(139);
  assert!(seeded.text("pull-request-state").starts_with("No changes"));
  seeded.pull_request(136);
  assert!(
    seeded
      .text("pull-request-state")
      .starts_with("The diff could not be read")
  );
  assert!(seeded.shows("pull-request-state-github"));
  seeded.pull_request(125);
  let too_large = seeded.text("pull-request-state");
  assert!(too_large.starts_with("Too large to show here"), "{too_large}");
  assert!(too_large.contains("412 files, +38,214 / −9,870"), "{too_large}");
  // A file longer than a diff keeps shows its first lines; the rest are counted at its end.
  seeded.pull_request(137);
  assert_eq!(seeded.text("changes-diff-path"), "tests/fixtures/error_codes.json");
}

#[test]
fn pull_request_page_keeps_the_diff_read_before_when_github_is_unreachable() {
  let mut seeded = Seeded::answering("ok");
  seeded.pull_request(142);
  seeded.mode("unreachable");
  seeded.window.press("pull-request-refresh");
  seeded.read_ended();
  assert!(seeded.text("pull-request-notice").starts_with("Couldn’t reach GitHub"));
  let context = seeded.text("pull-request-context");
  assert!(context.contains("GitHub unreachable"), "{context}");
  assert!(
    seeded.shows("changes-file:src/webhooks/sender.rs"),
    "the stored diff stays"
  );
}

#[test]
fn pull_request_page_crumbs_and_task_lead_back() {
  let mut seeded = Seeded::answering("ok");
  seeded.pull_request(142);
  seeded.window.press("pull-request-task");
  assert!(matches!(
    seeded.window.navigation().route_now(),
    Route::Project(_, ProjectPage::Task(..))
  ));
}
