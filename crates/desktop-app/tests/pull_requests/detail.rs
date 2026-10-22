//! A pull request's page: its header, tabs and toolbar, the changed files beside the selected file's diff, and
//! what stands in their place (`c07NAC`): no changes, too large, a diff Orchester cannot read, and GitHub not
//! answering with the diff read before kept under a notice. A long title is cut before its number; the
//! reading state stands centered in the body; a long file's cut row sends the rest to GitHub; the counts have
//! their digits grouped.

use lurq::app::events::ScrollPhase;
use orchester_app::routes::{ProjectPage, Route};

use crate::harness::{Seeded, nonzero};

impl Seeded {
  /// Pull request `number`'s page, once its read ended.
  fn pull_request(&mut self, number: u64) {
    self.go_to(number);
    self.read_ended();
  }

  /// Opens pull request `number`'s page without waiting for its read.
  fn go_to(&mut self, number: u64) {
    self.window.navigation().go(Route::Project(
      self.project,
      ProjectPage::PullRequest(nonzero(number), None),
    ));
    self.window.pass();
  }

  /// Turns the wheel over the diff toward its end until its last row shows.
  fn scroll_to_diff_end(&mut self) {
    let (x, y, width, height) = self.bounds("changes-diff");
    let (x, y) = (x + width / 2.0, y + height / 2.0);
    self.window.until("the diff's last row", |window| {
      window.tree.mouse_move(x, y);
      window.tree.scroll(x, y, 0.0, -20_000.0, ScrollPhase::Scroll);
      window.pass();
      window.tree.get_element_by_id("changes-diff-note").is_some()
    });
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
    "Agent checks and Overview are planned."
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

#[test]
fn pull_request_page_cuts_a_long_title_before_its_number() {
  let mut seeded = Seeded::answering("ok");
  seeded.pull_request(136);
  let (title, _, title_width, _) = seeded.bounds("pull-request-title");
  let (number, _, number_width, _) = seeded.bounds("pull-request-number");
  let (refresh, ..) = seeded.bounds("pull-request-refresh");
  let (github, _, github_width, _) = seeded.bounds("pull-request-github");
  assert!(
    title + title_width <= number,
    "the title ends at {} past #136 at {number}",
    title + title_width
  );
  assert!(number + number_width <= refresh, "#136 stays left of Refresh");
  assert!(github + github_width <= 1440.0, "Open in GitHub stays whole");
  assert_eq!(seeded.text("pull-request-number"), "#136");
}

#[test]
fn pull_request_page_header_leaves_18_over_its_tabs_and_10_inside_the_files_head() {
  let mut seeded = Seeded::answering("ok");
  seeded.pull_request(142);
  let (_, top, _, height) = seeded.bounds("pull-request-organization");
  let (_, tabs, ..) = seeded.bounds("pull-request-tabs");
  assert!(
    (tabs - (top + height) - 18.0).abs() < 0.5,
    "{} between",
    tabs - top - height
  );
  let (list, _, list_width, _) = seeded.bounds("pull-request-files");
  let (heading, ..) = seeded.bounds("pull-request-files-heading");
  let (field, _, field_width, _) = seeded.bounds("pull-request-find-file-box");
  assert!(
    (heading - list - 18.0).abs() < 0.5,
    "FILES starts {} in",
    heading - list
  );
  assert!(
    (field - list - 18.0).abs() < 0.5,
    "Find a file starts {} in",
    field - list
  );
  assert!(
    (list_width - field_width - 36.0).abs() < 0.5,
    "Find a file spans {field_width}"
  );
}

#[test]
fn pull_request_page_reading_stands_centered_in_the_body() {
  let mut seeded = Seeded::answering("slow");
  seeded.go_to(133);
  seeded.window.until("the reading state", |window| {
    window.tree.get_element_by_id("pull-request-reading-glyph").is_some()
  });
  let (body, _, width, _) = seeded.bounds("pull-request-reading");
  let middle = body + width / 2.0;
  for id in ["pull-request-reading-glyph", "pull-request-reading-title"] {
    let (x, _, part, _) = seeded.bounds(id);
    assert!((x + part / 2.0 - middle).abs() < 1.0, "{id} is centered");
  }
  seeded.read_ended();
}

#[test]
fn pull_request_page_long_file_cut_sends_the_rest_to_github() {
  let mut seeded = Seeded::answering("ok");
  seeded.pull_request(137);
  assert!(
    seeded.text("changes-diff-head").contains("+6,248"),
    "{}",
    seeded.text("changes-diff-head")
  );
  seeded.scroll_to_diff_end();
  let note = seeded.text("changes-diff-note");
  assert!(
    note.contains(
      "1,248 more changed lines are not shown; the file is too large. Open the pull request in GitHub to see them."
    ),
    "{note}"
  );
  assert!(seeded.shows("pull-request-cut-github"));
  seeded.pull_request(125);
  assert_eq!(seeded.text("pull-request-total"), "412 files · +38,214 / −9,870");
}
