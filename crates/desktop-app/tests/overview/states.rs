//! What the Overview shows in each of its states: the seeded project's tasks, figures, runs and what waits
//! for the person, all counted as `orchester_workspace::tasks::{board, attention, overview}` counts them; a
//! project without tasks; the tasks being read; and a reading that failed.

use std::{thread, time::Duration};

use orchester_app::routes::OverviewTab;
use orchester_workspace::tasks::{
  attention,
  board::{BoardTab, Figures},
  overview::recent_runs,
};

use crate::harness::Seeded;

#[test]
fn overview_shows_its_sections_from_the_project_feed() {
  let mut seeded = Seeded::overview("tasks", OverviewTab::Running);
  let tasks = seeded.tasks();
  assert_eq!(seeded.text("overview-eyebrow"), "PROJECT KEY KON");
  assert_eq!(seeded.text("overview-title"), "Kontur");
  for (tab, name) in [
    (BoardTab::Running, "running"),
    (BoardTab::NeedsYou, "needs_you"),
    (BoardTab::Queued, "queued"),
    (BoardTab::Failed, "failed"),
  ] {
    let label = seeded.text(&format!("overview-tab-{name}-label"));
    assert!(label.ends_with(&format!(" {}", tab.count(&tasks))), "{name}: {label}");
  }
  let shown: Vec<String> = seeded
    .ids("overview-row-")
    .into_iter()
    .map(|id| id.trim_start_matches("overview-row-").to_owned())
    .collect();
  let expected: Vec<String> = BoardTab::Running
    .rows(&tasks, 6, ", ")
    .iter()
    .map(|row| row.key.to_string())
    .collect();
  assert_eq!(shown, expected, "the Running tab's newest tasks, newest first");
  let items = attention::items(&tasks);
  assert!(!items.is_empty(), "the seed has tasks waiting for the person");
  assert_eq!(seeded.text("overview-needs-count"), items.len().to_string());
  assert_eq!(
    seeded
      .ids("overview-need-")
      .iter()
      .filter(|id| !id.contains("-action"))
      .count(),
    items.len()
  );
  let figures = Figures::of(&tasks, orchester_core::time::Timestamp::now().local_day_start());
  assert_eq!(
    seeded.text("overview-metric-needs-value"),
    figures.needs_you.to_string()
  );
  let recent = recent_runs(&tasks.tasks, &tasks.phases, 6);
  let events: Vec<String> = seeded.ids("overview-event-");
  assert_eq!(
    events,
    recent
      .iter()
      .map(|run| format!("overview-event-{}", run.key))
      .collect::<Vec<_>>()
  );
  assert!(seeded.text("overview-footer-count").contains("completed"));
  let notices = seeded.ids("overview-notice-");
  assert!(
    notices.iter().any(|id| id.ends_with("-unreadable")),
    "the seed's unreadable run has its notice: {notices:?}"
  );
  assert!(
    notices
      .iter()
      .all(|id| id.contains("-unreadable") || id.contains("-blocked")),
    "a notice's id names its kind, so a task's two notices stay apart: {notices:?}"
  );
}

#[test]
fn overview_counts_interrupted_runs_as_needing_the_person() {
  let mut seeded = Seeded::overview("tasks", OverviewTab::Running);
  let tasks = seeded.tasks();
  let figures = Figures::of(&tasks, orchester_core::time::Timestamp::now().local_day_start());
  assert!(
    figures.interrupted > 0,
    "without a runner, runs recorded as running are interrupted"
  );
  let interrupted = seeded
    .ids("overview-need-")
    .into_iter()
    .filter(|id| id.ends_with("-interrupted"))
    .count();
  assert_eq!(interrupted, figures.interrupted);
  assert_eq!(
    seeded.text("overview-metric-running-value"),
    (figures.running + figures.interrupted).to_string()
  );
  let detail = seeded.text("overview-metric-running-detail");
  assert!(detail.ends_with("interrupted"), "{detail}");
}

#[test]
fn overview_of_a_project_without_tasks_says_what_a_task_is() {
  let mut seeded = Seeded::overview("empty", OverviewTab::Running);
  assert_eq!(seeded.text("overview-empty-title"), "No tasks yet");
  assert!(seeded.shows("overview-empty-new-task"));
  assert_eq!(
    seeded.text("overview-empty-import-reason"),
    "Import resources isn't built in this app yet."
  );
  assert_eq!(seeded.text("overview-needs-count"), "0");
  assert_eq!(
    seeded.texts("overview-needs-empty")[..1],
    ["Nothing needs you right now.".to_owned()]
  );
  assert!(seeded.shows("overview-activity-empty"));
  assert!(!seeded.shows("overview-task-list"), "no tabs over nothing");
  assert_eq!(seeded.text("overview-metric-completed-value"), "0");
}

#[test]
fn overview_while_the_tasks_are_read_shows_dashes_and_readers_never_an_empty_page() {
  let mut seeded = Seeded::reading("tasks", true);
  let hold = seeded
    .window
    .launch
    .services()
    .expect("the session started")
    .hold
    .clone();
  seeded.go(OverviewTab::Running);
  seeded.until("the page", |seeded| seeded.shows("screen-overview"));
  for _ in 0..20 {
    seeded.window.tree.tick_futures();
    seeded.window.pass();
    thread::sleep(Duration::from_millis(5));
  }
  assert_eq!(seeded.text("overview-tab-running-label"), "Running —");
  assert_eq!(seeded.text("overview-tasks-reading"), "Reading Kontur's tasks…");
  assert_eq!(seeded.text("overview-activity-reading"), "Reading activity…");
  assert_eq!(seeded.text("overview-needs-count"), "—");
  assert_eq!(seeded.text("overview-metric-running-value"), "—");
  assert_eq!(seeded.text("overview-metric-needs-detail"), "Reading…");
  assert!(!seeded.shows("overview-empty") && !seeded.shows("overview-needs-empty"));
  hold.set(false);
  seeded.until("the tasks", |seeded| seeded.shows("overview-footer"));
  assert!(!seeded.shows("overview-tasks-reading"));
}

#[test]
fn overview_when_the_tasks_cannot_be_read_says_why_with_try_again() {
  let mut seeded = Seeded::reading("tasks-unreadable", false);
  seeded.go(OverviewTab::Running);
  seeded.until("the failure", |seeded| seeded.shows("overview-load-error"));
  let said = seeded.texts("overview-load-error");
  assert_eq!(said.first().map(String::as_str), Some("The tasks cannot be read"));
  assert_eq!(said.last().map(String::as_str), Some("Try again"));
  assert!(!seeded.shows("overview-needs-empty") && !seeded.shows("overview-empty"));
}

#[test]
fn overview_keeps_the_recovery_notice_above_its_heading() {
  let mut seeded = Seeded::overview("interrupted", OverviewTab::Running);
  seeded.until("the recovery notice", |seeded| seeded.shows("recovery-notice"));
  assert!(seeded.top("recovery-notice") < seeded.top("overview-header"));
  seeded.click("recovery-review");
  assert!(seeded.shows("recovery-notice"), "the list opens in place");
  assert!(seeded.top("recovery-notice") < seeded.top("overview-header"));
}
