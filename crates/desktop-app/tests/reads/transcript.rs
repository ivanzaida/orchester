//! A run's transcript growing reads again only what shows that run while it runs, and a burst of lines once per
//! pace; a run changing state reads again what shows runs. Each change still shows.

use std::{
  collections::BTreeMap,
  thread,
  time::{Duration, Instant},
};

use orchester_app::{
  project::feed::TRANSCRIPT_PACE,
  routes::{ProjectPage, TaskView},
};
use orchester_core::{
  id::RunId,
  name::Name,
  store::{RunStore, TaskStore},
  task::TaskUpdate,
};
use orchester_store::ProjectDb;

use super::Counted;

/// `key`'s page on `view` of the `conversation` scenario, where KON-12's run runs and KON-1's was merged.
///
/// As the project opens, its waits follower (`project::waits`) keeps the scenario's waits in the Orchestrator chat,
/// which moves the chat's count that a conversation's requests follow. It asks git first, so on a loaded machine it
/// may end after the page settled: the page waits for it, so it never lands amid a count of the test.
fn task_page(key: &str, view: TaskView, shown: &str) -> Counted {
  let mut counted = Counted::open("conversation");
  counted.show(ProjectPage::Task(key.parse().expect("a task key"), view), shown);
  let services = counted.window.launch.services().expect("the session started");
  let chats = services.runners.chats_of(&counted.root());
  counted
    .window
    .until("the waits kept in the Orchestrator chat", |_| *chats.borrow() > 0);
  counted.settle();
  counted
}

/// Gives `key` the title `title` in the project's database, as no screen does: only a read shows it.
fn retitle(counted: &Counted, key: &str, title: &str) {
  let mut db = ProjectDb::open(&counted.root()).expect("the project's database");
  let task = db
    .tasks()
    .expect("the tasks")
    .into_iter()
    .find(|task| task.key().to_string() == key)
    .expect("the task");
  let mut details = task.details().clone();
  details.title = Name::new(title).expect("a title");
  let update = db.update_task(task.id(), task.revision(), details).expect("retitled");
  assert!(matches!(update, TaskUpdate::Saved(_)), "retitled over the task as read");
}

fn title(counted: &mut Counted) -> String {
  counted.window.text("task-title")
}

fn counts(pairs: &[(&str, usize)]) -> BTreeMap<String, usize> {
  pairs.iter().map(|(read, count)| ((*read).to_owned(), *count)).collect()
}

#[test]
fn reads_a_transcript_line_again_only_where_its_run_runs() {
  let mut running = task_page("KON-12", TaskView::Conversation, "conversation-chat");
  retitle(&running, "KON-12", "Retry webhooks, renamed");
  running.report(false);
  let line = running.settle();
  // The context meter of its step at work reads its step's context use with the line.
  let read = [
    ("context use", 1),
    ("project usage", 1),
    ("requests", 1),
    ("task", 1),
    ("transcript", 1),
  ];
  assert_eq!(
    line,
    counts(&read),
    "the running run's page reads it, and the project's use today"
  );
  assert!(title(&mut running).contains("Retry webhooks, renamed"), "and shows it");

  let mut ended = task_page("KON-1", TaskView::Summary, "task-summary");
  retitle(&ended, "KON-1", "CI caching, renamed");
  ended.report(false);
  let line = ended.settle();
  assert_eq!(
    line,
    counts(&[("project usage", 1)]),
    "another run's lines read only the project's use today"
  );
  ended.report(true);
  let state = ended.settle();
  assert_eq!(
    state,
    counts(&[("project tasks", 1), ("project usage", 1), ("running", 1), ("task", 1)]),
    "the sidebar's runs under way read again with it"
  );
  assert!(
    title(&mut ended).contains("CI caching, renamed"),
    "a run's state reads the page again"
  );
}

#[test]
fn reads_a_burst_of_lines_once_per_pace_and_shows_the_last() {
  let mut counted = task_page("KON-12", TaskView::Summary, "task-summary");
  let start = Instant::now();
  for line in 0..20 {
    if line == 19 {
      retitle(&counted, "KON-12", "Retry webhooks, after the burst");
    }
    counted.report(false);
    counted.window.tree.tick_futures();
    counted.window.pass();
    thread::sleep(Duration::from_millis(10));
  }
  let burst = counted.settle();
  let paces = (start.elapsed().as_millis() / TRANSCRIPT_PACE.as_millis()) as usize;
  let reads = burst.get("task").copied().unwrap_or(0);
  assert!(
    (1..=paces + 1).contains(&reads),
    "{reads} reads of the task in {paces} paces"
  );
  let usage = burst.get("project usage").copied().unwrap_or(0);
  assert!(
    (1..=paces + 1).contains(&usage),
    "{usage} reads of today's use in {paces} paces"
  );
  assert_eq!(burst.len(), 2, "only the task and today's use are read: {burst:?}");
  assert!(
    title(&mut counted).contains("after the burst"),
    "the last line is read too"
  );
}

#[test]
fn reads_todays_use_on_a_line_while_one_of_the_projects_runs_runs() {
  // KON-12 runs; KON-1's page shows the project's limits and today's use in the status bar.
  let mut counted = task_page("KON-1", TaskView::Summary, "task-summary");
  counted.report(false);
  let line = counted.settle();
  assert_eq!(
    line.get("project usage"),
    Some(&1),
    "an agent's turn moves today's use: {line:?}"
  );
}

/// The latest run of `key`.
fn latest_run(counted: &Counted, key: &str) -> RunId {
  let db = ProjectDb::open(&counted.root()).expect("the project's database");
  let task = db
    .tasks()
    .expect("the tasks")
    .into_iter()
    .find(|task| task.key().to_string() == key)
    .expect("the task");
  let runs = db.runs_of_task(task.id()).expect("the task's runs");
  runs
    .into_iter()
    .filter_map(Result::ok)
    .last()
    .expect("a run of the task")
    .id()
}

#[test]
fn reads_a_running_tasks_page_again_on_its_runs_lines_not_another_runs() {
  // KON-12 runs: its Conversation reads the task, the run's transcript with its requests, and its step's context.
  let mut page = task_page("KON-12", TaskView::Conversation, "conversation-chat");
  let run = latest_run(&page, "KON-12");
  retitle(&page, "KON-12", "Retry webhooks, renamed");
  page.report_of(Some(RunId::generate()), false);
  let other = page.settle();
  assert_eq!(
    other,
    counts(&[("project usage", 1)]),
    "another run's line reads only the project's use today, no part of the page"
  );
  assert!(!title(&mut page).contains("renamed"), "the page was not read again");
  page.report_of(Some(run), false);
  let own = page.settle();
  for read in ["task", "transcript", "requests"] {
    assert_eq!(own.get(read), Some(&1), "its own run's line reads the {read}: {own:?}");
  }
  assert!(title(&mut page).contains("Retry webhooks, renamed"), "and shows it");
}
