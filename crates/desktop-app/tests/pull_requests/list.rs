//! The list: each tab's pull requests with their counts, a row's state, review, checks and task, the
//! footer, the search, the sort, and what a press on a row or its task opens.

use orchester_app::routes::{ProjectPage, Route, TaskTab, TaskView};
use orchester_workspace::pull_requests::list::PullRequestTab;

use crate::harness::{Seeded, nonzero};

#[test]
fn pull_requests_list_open_ones_with_state_review_checks_and_task() {
  let mut seeded = Seeded::answering("ok");
  seeded.open(PullRequestTab::Open);
  assert_eq!(seeded.text("pull-requests-sync-status"), "Synced just now");
  assert_eq!(seeded.text("pull-requests-repository"), "ivanzaida / kontur");
  assert_eq!(seeded.rows(), [142, 137, 139, 136, 134, 133]);
  let task = seeded.key("Add retry to webhook sender");
  let row = seeded.text("pull-request-row-142");
  for part in [
    "Add retry to webhook sender",
    "#142",
    task.as_str(),
    "→ main",
    "6 files · +133 / −15 · Updated 8 min ago",
    "Open",
    "Review required",
    "5 passed",
    "1 running",
    "Open in GitHub",
  ] {
    assert!(row.contains(part), "{part} in {row}");
  }
  assert!(seeded.text("pull-request-checks-137").contains("1 failed"));
  assert_eq!(seeded.text("pull-request-state-137"), "Open Changes requested");
  assert_eq!(seeded.text("pull-request-state-139"), "Draft No review required");
  assert_eq!(seeded.text("pull-request-task-134"), "No linked task");
  assert_eq!(seeded.text("pull-request-checks-133"), "No checks");
  assert_eq!(
    seeded.text("pull-requests-footer"),
    "6 open pull requests · From this project’s repository"
  );
}

#[test]
fn pull_requests_tabs_count_and_hold_their_pull_requests() {
  let mut seeded = Seeded::answering("ok");
  seeded.open(PullRequestTab::Open);
  let tabs = [
    ("open", "Open 6"),
    ("needs_review", "Needs your review 2"),
    ("agent_working", "Agent working 3"),
    ("merged", "Merged 3"),
    ("closed", "Closed 1"),
  ];
  for (name, label) in tabs {
    assert_eq!(seeded.text(&format!("pull-requests-tab-{name}")), label);
  }
  seeded.window.press("pull-requests-tab-needs_review");
  seeded.settled();
  assert_eq!(seeded.rows(), [142, 136]);
  assert_eq!(
    seeded.text("pull-requests-footer"),
    "2 of 6 open pull requests · From this project’s repository"
  );
  seeded.window.press("pull-requests-tab-agent_working");
  seeded.settled();
  assert_eq!(seeded.rows(), [142, 139, 136]);
  seeded.window.press("pull-requests-tab-merged");
  seeded.settled();
  assert_eq!(seeded.rows(), [131, 128, 127]);
  assert_eq!(seeded.text("pull-request-state-131"), "Merged Approved");
  assert!(
    seeded.text("pull-request-changes-131").ends_with("Merged 2 d ago"),
    "{}",
    seeded.text("pull-request-changes-131")
  );
  assert_eq!(
    seeded.text("pull-requests-footer"),
    "3 merged pull requests · From this project’s repository"
  );
  seeded.window.press("pull-requests-tab-closed");
  seeded.settled();
  assert_eq!(seeded.rows(), [125]);
}

#[test]
fn pull_requests_search_finds_numbers_branches_and_task_keys() {
  let mut seeded = Seeded::answering("ok");
  seeded.open(PullRequestTab::Open);
  seeded.window.set("pull-requests-search", "#134");
  assert_eq!(seeded.rows(), [134]);
  assert_eq!(
    seeded.text("pull-requests-footer"),
    "1 of 6 open pull requests · From this project’s repository"
  );
  let key = seeded.key("Typed errors per module");
  seeded.window.set("pull-requests-search", &key.to_lowercase());
  assert_eq!(seeded.rows(), [137]);
  seeded.window.set("pull-requests-search", "nothing like this");
  assert!(seeded.rows().is_empty());
  assert_eq!(seeded.text("pull-requests-no-rows-title"), "No pull request matches");
  seeded.window.press("pull-requests-clear-search");
  assert_eq!(seeded.rows().len(), 6);
}

#[test]
fn pull_requests_row_opens_the_pull_request_and_its_task_key_the_task() {
  let mut seeded = Seeded::answering("ok");
  seeded.open(PullRequestTab::Open);
  let key = seeded.key("Add retry to webhook sender");
  seeded.window.press("pull-request-task-142");
  assert_eq!(
    seeded.window.navigation().route_now(),
    Route::Project(
      seeded.project,
      ProjectPage::Task(key.parse().expect("a key"), TaskView::of(TaskTab::Summary))
    )
  );
  seeded.open(PullRequestTab::Open);
  seeded.window.press("pull-request-row-137");
  assert_eq!(
    seeded.window.navigation().route_now(),
    Route::Project(seeded.project, ProjectPage::PullRequest(nonzero(137)))
  );
  seeded.window.until("the pull request's page", |window| {
    window.tree.get_element_by_id("pull-request-header").is_some()
  });
}
