//! The feed of a project the tabs show: it reads the project's tasks, its limits and its runner, reads
//! the tasks again when a screen saved some or automation changed them, keeps the tasks last read when a
//! reading fails, and holds its first reading while QA holds reads.

mod support;

use std::{
  fs,
  path::Path,
  thread,
  time::{Duration, Instant},
};

use orchester_app::{
  project::{ProjectFeed, TasksState},
  routes::{ProjectPage, Route, TaskList},
};
use orchester_core::{
  id::IdempotencyKey,
  name::Name,
  task::{TaskDetails, TaskLimits},
};
use orchester_workspace::tasks::{
  data::{SaveTarget, save_draft_in},
  outside::Outside,
};
use support::{Window, project_id};

/// The Kontur window with its Tasks list shown, and Kontur's feed.
fn kontur_tasks() -> (Window, ProjectFeed) {
  let mut window = Window::open();
  window
    .navigation()
    .go(Route::Project(project_id(), ProjectPage::Tasks(TaskList::default())));
  window.until("Kontur to open", |window| window.texts("screen-tasks").is_some());
  let feed = window.projects().feed(project_id());
  (window, feed)
}

fn draft(repo: &Path, title: &str) {
  let details = TaskDetails {
    title: Name::new(title).expect("a title"),
    brief: String::new(),
    acceptance_criteria: Vec::new(),
    pipeline: None,
    limits: TaskLimits::NONE,
  };
  save_draft_in(repo, SaveTarget::New(IdempotencyKey::generate()), &details).expect("the draft is saved");
}

/// The titles of the tasks the feed holds; `None` while it holds none.
fn titles(feed: &ProjectFeed) -> Option<Vec<String>> {
  match feed.tasks.get_untracked() {
    TasksState::Ready(tasks) => Some(
      tasks
        .tasks
        .iter()
        .map(|task| task.details().title.as_str().to_owned())
        .collect(),
    ),
    TasksState::Loading | TasksState::Failed(_) => None,
  }
}

fn wait_for_titles(window: &mut Window, feed: &ProjectFeed, expected: &[&str]) {
  window.until(&format!("the tasks {expected:?}"), |_| {
    titles(feed).is_some_and(|mut titles| {
      titles.sort();
      titles == expected
    })
  });
}

#[test]
fn feed_loads_the_tasks_limits_and_runner() {
  let mut window = Window::open();
  draft(&window.data.repo("repo-kontur"), "Retry with jitter");
  window
    .navigation()
    .go(Route::Project(project_id(), ProjectPage::Tasks(TaskList::default())));
  window.until("Kontur to open", |window| window.texts("screen-tasks").is_some());
  let feed = window.projects().feed(project_id());
  wait_for_titles(&mut window, &feed, &["Retry with jitter"]);
  window.until("the limits", |_| feed.usage.get_untracked().is_some());
  window.until("the runner", |_| feed.status.get_untracked().is_some());
  assert_eq!(feed.stale.get_untracked(), None);
}

#[test]
fn feed_reads_again_when_a_change_is_counted() {
  let (mut window, feed) = kontur_tasks();
  wait_for_titles(&mut window, &feed, &[]);
  let repo = window.data.repo("repo-kontur");
  draft(&repo, "Saved by a screen");
  feed.saved();
  wait_for_titles(&mut window, &feed, &["Saved by a screen"]);
  draft(&repo, "Saved by automation");
  let services = window.launch.services().expect("the session started").clone();
  services.changes.record(project_id(), Outside::Tasks);
  wait_for_titles(&mut window, &feed, &["Saved by a screen", "Saved by automation"]);
  assert_eq!(feed.outside.get_untracked().tasks, 1);
}

#[test]
fn feed_stale_keeps_the_last_tasks_read() {
  let (mut window, feed) = kontur_tasks();
  let repo = window.data.repo("repo-kontur");
  draft(&repo, "Kept on screen");
  feed.saved();
  wait_for_titles(&mut window, &feed, &["Kept on screen"]);
  window.until("the limits", |_| feed.usage.get_untracked().is_some());
  let (data, away) = (repo.join(".orchester"), repo.join(".orchester-away"));
  move_when_closed(&data, &away);
  feed.saved();
  window.until("the failed reading", |_| feed.stale.get_untracked().is_some());
  assert_eq!(
    titles(&feed),
    Some(vec!["Kept on screen".to_owned()]),
    "the tasks last read stay"
  );
  move_when_closed(&away, &data);
  draft(&repo, "Read once it is back");
  feed.saved();
  window.pass();
  assert!(
    feed.stale.get_untracked().is_some(),
    "the notice stays while the next reading is under way"
  );
  wait_for_titles(&mut window, &feed, &["Kept on screen", "Read once it is back"]);
  assert_eq!(feed.stale.get_untracked(), None, "the good reading ended the notice");
}

/// Moves the folder `from` to `to` once no reading has a file in it open: Windows refuses to move a
/// folder while a file in it is open.
fn move_when_closed(from: &Path, to: &Path) {
  let deadline = Instant::now() + Duration::from_secs(10);
  while let Err(error) = fs::rename(from, to) {
    assert!(Instant::now() < deadline, "cannot move {}: {error}", from.display());
    thread::sleep(Duration::from_millis(20));
  }
}

#[test]
fn feed_hold_reads_keeps_the_first_reading_loading() {
  let mut window = Window::open();
  let hold = window.launch.services().expect("the session started").hold.clone();
  hold.set(true);
  window
    .navigation()
    .go(Route::Project(project_id(), ProjectPage::Tasks(TaskList::default())));
  window.until("Kontur to open", |window| window.texts("screen-tasks").is_some());
  let feed = window.projects().feed(project_id());
  window.until("the runner", |_| feed.status.get_untracked().is_some());
  for _ in 0..20 {
    window.tree.tick_futures();
    window.pass();
    thread::sleep(Duration::from_millis(5));
  }
  assert_eq!(feed.tasks.get_untracked(), TasksState::Loading, "the reading is held");
  assert_eq!(feed.usage.get_untracked(), None);
  hold.set(false);
  wait_for_titles(&mut window, &feed, &[]);
}
