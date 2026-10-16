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
  name::Name,
  store::TaskStore,
  task::TaskUpdate,
};
use orchester_store::ProjectDb;

use super::Counted;

/// `key`'s page on `view` of the `conversation` scenario, where KON-12's run runs and KON-1's was merged.
fn task_page(key: &str, view: TaskView, shown: &str) -> Counted {
  let mut counted = Counted::open("conversation");
  counted.show(ProjectPage::Task(key.parse().expect("a task key"), view), shown);
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
  eprintln!("KON-12 Conversation, a transcript line: {line:?}");
  assert_eq!(
    line,
    counts(&[("requests", 1), ("task", 1), ("transcript", 1)]),
    "the running run's page reads it, the project's usage does not"
  );
  assert!(title(&mut running).contains("Retry webhooks, renamed"), "and shows it");

  let mut ended = task_page("KON-1", TaskView::Summary, "task-summary");
  retitle(&ended, "KON-1", "CI caching, renamed");
  ended.report(false);
  let line = ended.settle();
  eprintln!("KON-1 Summary, another run's transcript line: {line:?}");
  assert_eq!(line, counts(&[]), "another run's lines read nothing");
  ended.report(true);
  let state = ended.settle();
  eprintln!("KON-1 Summary, a run's state: {state:?}");
  assert_eq!(
    state,
    counts(&[("project tasks", 1), ("project usage", 1), ("task", 1)])
  );
  assert!(title(&mut ended).contains("CI caching, renamed"), "a run's state reads the page again");
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
  eprintln!("KON-12 Summary, 20 lines 10 ms apart: {burst:?} in {paces} paces");
  let reads = burst.get("task").copied().unwrap_or(0);
  assert!((1..=paces + 1).contains(&reads), "{reads} reads of the task");
  assert_eq!(burst.len(), 1, "only the task is read: {burst:?}");
  assert!(title(&mut counted).contains("after the burst"), "the last line is read too");
}
