//! A run's merge attempt page while its attempts are read (ORC-915, `Review - v2` 13i `yHy6H`, rule 26 `rwJfp` of
//! `b6M3lS`), on the QA seed's `merge-attempts` scenario with QA's `hold_reads` holding the page's read: read for the
//! first time, the crumbs, the title and *Copy details* show at once and what is read is bones, the eight stages
//! named, with no bar; the parts stand where they will stand, so nothing moves when the read lands; and opened again
//! the page shows the attempts last read, with its bar, while it reads them anew. Needs `git`, as the seed does.
//! `cargo test -p orchester-app --test merge_attempt_reading merge_attempt_reading`.

#[path = "../examples/seed/kontur/mod.rs"]
#[allow(dead_code, reason = "the seed's helpers serve every scenario; these tests seed one")]
mod kontur;
mod support;

use std::{path::PathBuf, time::Duration};

use orchester_app::{
  project::TasksState,
  routes::{ProjectPage, Route, TaskList},
};
use orchester_core::{
  id::ProjectId,
  store::{RunStore, TaskStore},
  time::Timestamp,
};
use orchester_store::ProjectDb;

use crate::{
  kontur::{SeedRequest, scenarios},
  support::{Data, Window},
};

/// How long a read with git may take on a loaded machine.
const GIT_WORK: Duration = Duration::from_secs(60);
/// The task whose run the tests open: its second attempt was refused at the integration lock.
const TITLE: &str = "Add retry to webhook sender";

/// Where a part of the page stands: its left, top, width and height.
type Place = (f32, f32, f32, f32);

struct Page {
  window: Window,
  project: ProjectId,
  repo: PathBuf,
}

impl Page {
  fn new() -> Self {
    let data = Data::new();
    let repo = data.dir.path().join("repo-merge-attempts");
    let request = SeedRequest {
      data_dir: data.dir.path().to_path_buf(),
      repo: repo.clone(),
      scenario: scenarios::named("merge-attempts").expect("a known scenario"),
      fake_codex: None,
      now: Timestamp::now(),
    };
    let seeded = kontur::seed(&request).expect("the scenario seeds");
    let launch = data.launch();
    let mut window = Window::of(data, launch);
    window.tree.resize(1440, 1400);
    Self {
      window,
      project: seeded.project,
      repo,
    }
  }

  /// The route of the attempt page of `TITLE`'s run.
  fn route(&self) -> Route {
    let db = ProjectDb::open(&self.repo).expect("the project database");
    let task = db
      .tasks()
      .expect("the tasks")
      .into_iter()
      .find(|task| task.details().title.as_str() == TITLE)
      .expect("the scenario seeds the task");
    let run = db
      .runs_of_task(task.id())
      .expect("the runs")
      .into_iter()
      .find_map(Result::ok)
      .expect("its run");
    Route::Project(self.project, ProjectPage::TaskMerge(task.key().clone(), run.number()))
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

  fn hold(&mut self, held: bool) {
    self
      .window
      .launch
      .services()
      .expect("the session started")
      .hold
      .set(held);
  }

  /// Holds the reads and opens the page, which then shows its bones.
  fn open_held(&mut self) {
    self.hold(true);
    let route = self.route();
    self.window.navigation().go(route);
    self.window.until("the bones", |window| {
      window.tree.get_element_by_id("merge-attempt-reading").is_some()
    });
  }

  /// Lets the reads go and waits until the page is read.
  fn until_read(&mut self) {
    self.hold(false);
    self.window.until_within(GIT_WORK, "the attempt page", |window| {
      window.texts("merge-attempt-status-text").is_some()
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

  fn shows(&mut self, id: &str) -> bool {
    self.window.tree.get_element_by_id(id).is_some()
  }
}

#[test]
fn merge_attempt_reading_shows_the_title_and_bones_with_the_stages_named_and_no_bar() {
  let mut page = Page::new();
  page.tasks_read();
  page.open_held();
  assert_eq!(
    page.window.text("merge-attempt-title"),
    TITLE,
    "the task's title is known"
  );
  assert_eq!(page.window.text("merge-attempt-copy-details"), "Copy details");
  assert!(
    !page.shows("merge-attempt-status-text") && page.shows("merge-attempt-what-reading"),
    "what is read is bones"
  );
  let names: Vec<String> = (1..=8)
    .map(|stage| {
      page
        .window
        .texts(&format!("merge-attempt-reading-check-{stage}"))
        .unwrap_or_else(|| panic!("check {stage} stands"))
        .join(" ")
    })
    .collect();
  for (name, wanted) in names.iter().zip([
    "Project database",
    "Integration lock",
    "Scratch folder",
    "Checkout identity",
    "Git metadata",
    "Result already in Git",
    "Merge",
    "Record",
  ]) {
    assert!(name.contains(wanted), "{name} names {wanted}");
  }
  assert!(
    page
      .window
      .text("merge-attempt-log-folder")
      .contains("Runner log folder"),
    "the log folder needs nothing read"
  );
  assert!(!page.shows("merge-attempt-bar"), "the bar comes with the read");
}

#[test]
fn merge_attempt_reading_moves_nothing_when_the_attempts_are_read() {
  let mut page = Page::new();
  page.tasks_read();
  page.open_held();
  let header = [
    "merge-attempt-crumbs-row",
    "merge-attempt-header",
    "merge-attempt-status",
  ];
  let reading: Vec<Place> = header.iter().map(|id| page.place(id)).collect();
  let checks: Vec<Place> = (1..=8)
    .map(|stage| page.place(&format!("merge-attempt-reading-check-{stage}")))
    .collect();
  let facts = page.place("merge-attempt-facts");
  page.until_read();
  for ((id, was), now) in header.iter().zip(&reading).zip(header.iter().map(|id| page.place(id))) {
    // The status line only grows at its end, where its words and the run come with the read.
    let width = |place: &Place| if *id == "merge-attempt-status" { 0.0 } else { place.2 };
    assert_eq!(
      (was.0, was.1, width(was), was.3),
      (now.0, now.1, width(&now), now.3),
      "{id} stays where it was"
    );
  }
  let now = page.place("merge-attempt-facts");
  assert_eq!(
    (facts.0, facts.2),
    (now.0, now.2),
    "the facts keep their place and width"
  );
  for (stage, was) in (1..=8).zip(&checks) {
    let now = page.place(&format!("merge-attempt-checks-{stage}"));
    assert_eq!(
      (was.0, was.2),
      (now.0, now.2),
      "check {stage} keeps its place and width"
    );
    // The stage that refused grows by its cause, which is not known while the page is read.
    if stage != 2 {
      assert_eq!(was.3, now.3, "check {stage} keeps its height");
    }
  }
}

#[test]
fn merge_attempt_reading_a_page_opened_again_shows_the_attempts_last_read_with_its_bar() {
  let mut page = Page::new();
  page.tasks_read();
  let route = page.route();
  page.window.navigation().go(route.clone());
  page.window.until_within(GIT_WORK, "the attempt page", |window| {
    window.texts("merge-attempt-status-text").is_some()
  });
  page.tasks_read();
  page.hold(true);
  page.window.navigation().go(route);
  page
    .window
    .until("the page", |window| window.texts("merge-attempt-status-text").is_some());
  assert!(
    !page.shows("merge-attempt-reading"),
    "the attempts last read stand in while they are read anew"
  );
  assert_eq!(page.window.text("merge-attempt-status-text"), "Not merged into main");
  assert_eq!(page.window.text("merge-attempt-try-again"), "Try again");
  assert!(page.shows("merge-attempt-bar"));
  page.hold(false);
}
