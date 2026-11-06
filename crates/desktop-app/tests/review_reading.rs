//! A task's review page while its run's changes are read (ORC-802, `Review - v2` `j1Drb` 10 `Z655x`, rule 9 of
//! `b6M3lS`), on the QA seed's `review-list` scenario with QA's `hold_reads` holding the page's read: opened from the
//! Review list, the crumbs, the title with its buttons and the status line are the row's and only the files and the
//! diff are bones; opened directly, the title and the status are bones too; and in both the parts stand where they will
//! stand, so nothing moves when the read lands. Needs `git`, as the seed does.
//! `cargo test -p orchester-app --test review_reading review_reading`.

#[path = "../examples/seed/kontur/mod.rs"]
#[allow(dead_code, reason = "the seed's helpers serve every scenario; these tests seed one")]
mod kontur;
mod support;

use std::time::Duration;

use orchester_app::{
  project::TasksState,
  routes::{ProjectPage, Route, TaskList},
};
use orchester_core::{id::ProjectId, project::TaskKey, time::Timestamp};

use crate::{
  kontur::{SeedRequest, SeededTask, scenarios},
  support::{Data, Window},
};

/// How long a read with git may take on a loaded machine.
const GIT_WORK: Duration = Duration::from_secs(60);

/// Where each part of the page that must not move stands: its left, top, width and height.
type Place = (f32, f32, f32, f32);

/// The parts of the header, which stand where they stand once the page is read.
const HEADER: [&str; 5] = [
  "review-crumbs-row",
  "merge-attempt-header",
  "review-open-task",
  "review-approve",
  "review-status",
];

struct Page {
  window: Window,
  project: ProjectId,
  tasks: Vec<SeededTask>,
}

impl Page {
  fn new() -> Self {
    let data = Data::new();
    let repo = data.dir.path().join("repo-review-list");
    let request = SeedRequest {
      data_dir: data.dir.path().to_path_buf(),
      repo,
      scenario: scenarios::named("review-list").expect("a known scenario"),
      fake_codex: None,
      now: Timestamp::now(),
    };
    let seeded = kontur::seed(&request).expect("the scenario seeds");
    let launch = data.launch();
    let mut window = Window::of(data, launch);
    window.tree.resize(1440, 900);
    Self {
      window,
      project: seeded.project,
      tasks: seeded.tasks,
    }
  }

  fn key(&self, title: &str) -> TaskKey {
    let task = self
      .tasks
      .iter()
      .find(|task| task.title == title)
      .unwrap_or_else(|| panic!("the scenario seeds {title}"));
    task.key.parse().expect("a task key")
  }

  /// Opens the project's Tasks and waits until its tasks are read, so only the page's own read is left to wait for.
  fn tasks_read(&mut self) {
    self
      .window
      .navigation()
      .go(Route::Project(self.project, ProjectPage::Tasks(TaskList::default())));
    let feed = self.window.projects().feed(self.project);
    self.window.until_within(GIT_WORK, "the tasks", |_| {
      matches!(feed.tasks.get_untracked(), TasksState::Ready(_))
    });
  }

  /// Opens Review's list and waits until it is read, so the project's feed keeps it.
  fn list_read(&mut self) {
    self
      .window
      .navigation()
      .go(Route::Project(self.project, ProjectPage::Review));
    self.window.until_within(GIT_WORK, "the list", |window| {
      window.tree.get_element_by_id("review-list").is_some()
    });
  }

  /// Holds the reads, opens the review of `key` and waits for its bones.
  fn open_held(&mut self, key: &TaskKey) {
    self.hold(true);
    self
      .window
      .navigation()
      .go(Route::Project(self.project, ProjectPage::TaskReview(key.clone())));
    self.window.until("the bones", |window| {
      window.tree.get_element_by_id("review-reading-columns").is_some()
    });
  }

  fn hold(&mut self, held: bool) {
    self
      .window
      .launch
      .services()
      .expect("the session started")
      .hold
      .set(held);
  }

  /// Lets the reads go and waits until the page shows the run's diff.
  fn until_read(&mut self) {
    self.hold(false);
    self.window.until_within(GIT_WORK, "the review page", |window| {
      window.tree.get_element_by_id("review-diff").is_some()
    });
  }

  fn place(&mut self, id: &str) -> Place {
    let bounds = self
      .window
      .tree
      .get_element_by_id_mut(id)
      .and_then(|element| element.bounds())
      .unwrap_or_else(|| panic!("{id} is laid out"));
    (bounds.x, bounds.y, bounds.width, bounds.height)
  }

  fn places(&mut self, ids: &[&str]) -> Vec<Place> {
    ids.iter().map(|id| self.place(id)).collect()
  }

  fn shows(&mut self, id: &str) -> bool {
    self.window.tree.get_element_by_id(id).is_some()
  }
}

#[test]
fn review_reading_from_the_list_shows_the_crumbs_title_buttons_and_status_at_once() {
  let mut page = Page::new();
  page.list_read();
  let key = page.key("Sign webhook payloads");
  page.open_held(&key);
  assert_eq!(
    page.window.texts("review-crumbs"),
    Some(vec!["Review".to_owned(), "/".to_owned(), format!("{key} · run 1")]),
    "the run is the row's"
  );
  assert_eq!(page.window.text("merge-attempt-title"), "Sign webhook payloads");
  assert_eq!(page.window.text("review-status-text"), "Waiting for your review");
  let meta = page.window.text("review-status-meta");
  assert!(
    meta.contains("run #1") && meta.contains("every step passed") && meta.contains("1 file"),
    "{meta}"
  );
  assert!(!meta.contains("head "), "its head is not read yet: {meta}");
  assert!(page.shows("review-files-reading") && page.shows("review-diff-reading"));
  assert!(!page.shows("review-files") && !page.shows("review-diff"));
  assert!(page.shows("review-open-task") && page.shows("review-approve"));
}

#[test]
fn review_reading_opened_directly_has_bones_for_the_title_and_the_status_too() {
  let mut page = Page::new();
  page.tasks_read();
  let key = page.key("Sign webhook payloads");
  page.open_held(&key);
  assert_eq!(
    page.window.texts("review-crumbs"),
    Some(vec!["Review".to_owned(), "/".to_owned(), key.to_string()]),
    "the key alone, as the run is not known"
  );
  assert!(page.shows("review-title-reading") && page.shows("review-status-reading"));
  assert!(!page.shows("merge-attempt-title") && !page.shows("review-status-text"));
  assert!(page.shows("review-open-task") && page.shows("review-approve"));
}

#[test]
fn review_reading_moves_nothing_when_the_run_is_read() {
  for opened_from_the_list in [true, false] {
    let mut page = Page::new();
    if opened_from_the_list {
      page.list_read();
    } else {
      page.tasks_read();
    }
    let key = page.key("Sign webhook payloads");
    page.open_held(&key);
    let reading = page.places(&HEADER);
    let [files, diff] = ["review-files-reading", "review-diff-reading"].map(|id| page.place(id));
    page.until_read();
    let read = page.places(&HEADER);
    let (read_files, read_diff) = (page.place("review-files"), page.place("review-diff"));
    for ((id, was), now) in HEADER.iter().zip(&reading).zip(&read) {
      // The crumbs and the status line only grow at their end, where the run, the head and the base come with the read.
      let grows = ["review-crumbs-row", "review-status"].contains(id);
      let width = |place: &Place| if grows { 0.0 } else { place.2 };
      assert_eq!(
        (was.0, was.1, width(was), was.3),
        (now.0, now.1, width(now), now.3),
        "{id} stays where it was (opened from the list: {opened_from_the_list})"
      );
    }
    assert_eq!(
      (files.0, files.1, files.2),
      (read_files.0, read_files.1, read_files.2),
      "the files column keeps its place and width"
    );
    assert_eq!(
      (diff.0, diff.1, diff.2),
      (read_diff.0, read_diff.1, read_diff.2),
      "the diff keeps its place and width"
    );
  }
}
