//! What the Overview shows in each of its states: the seeded project's tasks, figures, runs and what waits
//! for the person, all counted as `orchester_workspace::tasks::{board, attention, overview}` counts them; a
//! project without tasks; the tasks being read; and a reading that failed.

use std::{thread, time::Duration};

use orchester_app::routes::OverviewTab;
use orchester_core::run::{InterruptCause, TaskPhase};
use orchester_exec::programs::Programs;
use orchester_workspace::tasks::{
  attention::{self, NeedKind},
  board::{BoardTab, Figures},
  checkout::Checkout,
  outside::Outside,
  overview::recent_runs,
  ready_to_run::ProjectReadiness,
};

use crate::harness::Seeded;

#[test]
fn overview_shows_its_sections_from_the_project_feed() {
  let mut seeded = Seeded::overview("tasks", OverviewTab::Running);
  let tasks = seeded.tasks();
  assert!(seeded.text("overview-eyebrow").starts_with("PROJECT KEY KON"));
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
  // A run recorded interrupted waits for the person; one that reads interrupted only because no runner
  // looks after it does not: a runner records it once one does.
  let recorded = attention::items(&tasks)
    .iter()
    .filter(|item| item.kind == NeedKind::Interrupted)
    .count();
  let interrupted = seeded
    .ids("overview-need-")
    .into_iter()
    .filter(|id| id.ends_with("-interrupted"))
    .count();
  assert_eq!(interrupted, recorded);
  assert_eq!(
    seeded.text("overview-metric-running-value"),
    (figures.running + figures.interrupted).to_string()
  );
  let detail = seeded.text("overview-metric-running-detail");
  assert!(detail.ends_with("interrupted"), "{detail}");
}

#[test]
fn overview_needs_you_counts_what_the_title_bar_pill_counts() {
  let mut seeded = Seeded::overview("tasks", OverviewTab::Running);
  let tasks = seeded.tasks();
  let count = tasks.needs_you();
  assert!(count > 0, "the seed has things waiting for the person");
  assert_eq!(seeded.text("overview-needs-count"), count.to_string());
  assert_eq!(seeded.text("overview-metric-needs-value"), count.to_string());
  seeded.until("the pill", |seeded| seeded.shows("needs-you-pill"));
  let pill = seeded.text("needs-you-pill");
  assert!(pill.starts_with(&format!("{count} ")), "{pill} against {count}");
}

#[test]
fn overview_of_a_project_without_tasks_says_what_a_task_is() {
  let mut seeded = Seeded::overview("empty", OverviewTab::Running);
  assert_eq!(seeded.text("overview-empty-title"), "No tasks yet");
  assert!(seeded.shows("overview-empty-new-task"));
  assert!(seeded.shows("overview-empty-import"));
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
fn overview_of_a_project_without_tasks_lists_what_is_ready_to_run() {
  let mut seeded = Seeded::overview("empty", OverviewTab::Running);
  seeded.until("the readiness", |seeded| seeded.shows("overview-ready-agents"));
  let readiness = ProjectReadiness::load_in(&seeded.repo).expect("the readiness reads");
  assert_eq!(seeded.texts("overview-ready")[0], "Ready to run");
  let agents = readiness.agents.join(", ");
  assert!(!agents.is_empty(), "the seed publishes its agents");
  assert_eq!(seeded.text("overview-ready-agents-detail"), agents);
  for (check, name) in readiness.checks(", ").iter().zip(["agents", "pipeline", "connection"]) {
    let line = seeded.texts(&format!("overview-ready-{name}"));
    assert_eq!(line.len(), 2, "{name}: its name and what was found: {line:?}");
    let detail = seeded.text(&format!("overview-ready-{name}-detail"));
    assert!(!detail.is_empty() && !detail.contains("overview."), "{name}: {detail}");
    let state = if check.ready { "ready" } else { "missing" };
    assert!(
      seeded.shows(&format!("overview-ready-{name}-{state}")),
      "{name} is {state}"
    );
  }
  assert!(
    seeded.top("overview-ready") > seeded.top("overview-empty-new-task"),
    "the list is under the card's actions"
  );
}

#[test]
fn overview_ready_to_run_keeps_its_lines_with_the_error_when_reading_again_fails() {
  let mut seeded = Seeded::overview("empty", OverviewTab::Running);
  seeded.until("the readiness", |seeded| seeded.shows("overview-ready-agents"));
  let agents = seeded.text("overview-ready-agents-detail");
  let db =
    rusqlite::Connection::open(seeded.repo.join(".orchester").join("orchester.db")).expect("the project database");
  let changes = seeded
    .window
    .launch
    .services()
    .expect("the session started")
    .changes
    .clone();
  db.execute_batch("ALTER TABLE connections RENAME TO connections_hidden")
    .expect("hide the connections");
  changes.record(seeded.project, Outside::Connections);
  seeded.until("the failed reading", |seeded| seeded.shows("overview-ready-error"));
  assert_eq!(
    seeded.text("overview-ready-agents-detail"),
    agents,
    "the lines read before stay"
  );
  assert!(seeded.top("overview-ready-error") > seeded.top("overview-ready-connection"));
  db.execute_batch("ALTER TABLE connections_hidden RENAME TO connections")
    .expect("restore the connections");
  changes.record(seeded.project, Outside::Connections);
  seeded.until("the reading again", |seeded| !seeded.shows("overview-ready-error"));
  assert_eq!(seeded.text("overview-ready-agents-detail"), agents);
}

#[test]
fn overview_names_a_task_whose_run_passed_every_step_ready_to_merge() {
  let mut seeded = Seeded::overview("task-states", OverviewTab::NeedsYou);
  let tasks = seeded.tasks();
  let items = attention::items(&tasks);
  let rows = BoardTab::NeedsYou.rows(&tasks, 6, ", ");
  let merging = rows
    .iter()
    .find(|row| row.phase == Some(TaskPhase::Succeeded))
    .expect("the seed has a task whose run passed every step among the newest waiting");
  let key = &merging.key;
  let status = seeded.text(&format!("overview-status-{key}"));
  assert_eq!(status, "Ready to merge · Deliver", "{key} (`XirGV`)");
  let item = items
    .iter()
    .find(|item| item.task().is_some_and(|task| task.id == merging.id))
    .expect("the task waits in Needs you");
  assert_eq!(merging.detail, item.title, "the row says what waits, as its card does");
  let detail = seeded.text(&format!("overview-detail-{key}"));
  assert!(detail.contains("ready to merge"), "{key}: {detail}");
}

#[test]
fn overview_says_the_token_limit_a_run_stopped_at() {
  let mut seeded = Seeded::overview("task-states", OverviewTab::Running);
  let tasks = seeded.tasks();
  let limited = attention::items(&tasks)
    .into_iter()
    .find(|item| item.title.key == "overview.needs.title.tokenLimit")
    .expect("the seed has a run stopped at its token limit");
  let limit = &limited
    .title
    .args
    .iter()
    .find(|(name, _)| *name == "limit")
    .expect("the limit")
    .1;
  let key = &limited.task().expect("its task").key;
  let card = seeded
    .ids("overview-need-")
    .into_iter()
    .find(|id| id.ends_with(&format!("-{key}-limit")))
    .expect("the limit's card");
  let said = seeded.text(&card);
  assert!(said.contains(&format!("{limit}-token limit")), "{said}");
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

/// The compact notice says why the runs stopped (`xPTrL` in `JAIYi`), as the open list does.
#[test]
fn overview_recovery_notice_says_why_the_runs_stopped() {
  let mut seeded = Seeded::overview("interrupted", OverviewTab::Running);
  seeded.until("the recovery notice", |seeded| seeded.shows("recovery-review"));
  let services = seeded.window.launch.services().expect("the session started").clone();
  let runs = services.recovery.state().runs_of(seeded.project);
  let quit = runs.iter().all(|run| run.cause == InterruptCause::QuitNow);
  let why = if quit {
    "You chose Quit now."
  } else {
    "Orchester stopped unexpectedly."
  };
  let said = seeded.text("recovery-notice");
  assert!(
    said.contains(&format!("{why} Their worktrees and commits are kept.")),
    "{said}"
  );
}

/// The line over the title names what git tells of the checkout: the seed's repository has no network
/// `origin`, so its branch follows the key.
#[test]
fn overview_names_the_checkouts_branch_over_the_title() {
  let mut seeded = Seeded::overview("tasks", OverviewTab::Running);
  let checkout = Checkout::read(&Programs::processes(), &seeded.repo);
  assert_eq!(checkout.repository, None, "the seed has no network origin");
  let branch = checkout.branch.expect("the seed checks out a branch");
  let expected = format!("PROJECT KEY KON · {branch}");
  seeded.until("git's branch", |seeded| seeded.text("overview-eyebrow") == expected);
}

/// The status bar's *Review* (`uZTnL`) opens the list through the session's recovery, as the notice's own
/// *Review runs* does; the bar's item itself is `status_bar`'s (this window has no runner, whose problem
/// takes the bar's slot first, as `KGpMB` draws).
#[test]
fn overview_recovery_list_opens_from_the_session_as_the_status_bar_asks_and_hides_again() {
  let mut seeded = Seeded::overview("interrupted", OverviewTab::Running);
  seeded.until("the recovery notice", |seeded| seeded.shows("recovery-review"));
  let services = seeded.window.launch.services().expect("the session started").clone();
  assert_eq!(services.recovery.state().undecided(seeded.project), 3);
  services.recovery.review(seeded.project, true);
  seeded.until("the list", |seeded| seeded.shows("recovery-hide"));
  assert!(!seeded.shows("recovery-review"));
  seeded.click("recovery-hide");
  seeded.until("the compact notice", |seeded| seeded.shows("recovery-review"));
  assert!(!services.recovery.state().reviewing(seeded.project));
  assert_eq!(
    services.recovery.state().undecided(seeded.project),
    3,
    "the runs wait until the person decides"
  );
}
