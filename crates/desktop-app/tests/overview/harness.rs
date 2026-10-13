//! A window on a seeded project's Overview, tall enough that the whole page is laid out, and what the tests
//! read off it. No runner serves the project (its program does not exist), so a run recorded as running
//! reads as interrupted, and the tasks the page counts are the project's as they stand without one.

use std::path::{Path, PathBuf};

use lurq::app::events::MouseButton;
use orchester_app::routes::{OverviewTab, ProjectPage, Route};
use orchester_core::{id::ProjectId, time::Timestamp};
use orchester_workspace::tasks::data::ProjectTasks;

use crate::{
  kontur::{self, SeedRequest, scenarios},
  support::{Data, Window},
};

/// Room for the whole page.
const TALL: (u32, u32) = (1440, 2000);

pub struct Seeded {
  pub window: Window,
  pub project: ProjectId,
  pub repo: PathBuf,
}

impl Seeded {
  /// `scenario` seeded into a new data folder, and its window, its feeds' readings held while `held`.
  pub fn reading(scenario: &str, held: bool) -> Self {
    Self::prepared(scenario, held, |_| {})
  }

  /// `scenario` seeded, then changed by `prepare` in its repository before the app opens it.
  fn prepared(scenario: &str, held: bool, prepare: impl FnOnce(&Path)) -> Self {
    let data = Data::new();
    let request = SeedRequest {
      data_dir: data.dir.path().to_path_buf(),
      repo: data.dir.path().join(format!("repo-{scenario}")),
      scenario: scenarios::named(scenario).expect("a known scenario"),
      fake_codex: None,
      now: Timestamp::now(),
    };
    let seeded = kontur::seed(&request).expect("the scenario seeds");
    prepare(&seeded.repo);
    let launch = data.launch();
    launch.services().expect("the session started").hold.set(held);
    let mut window = Window::of(data, launch);
    window.tree.resize(TALL.0, TALL.1);
    Self {
      window,
      project: seeded.project,
      repo: seeded.repo,
    }
  }

  /// `scenario` with its Overview shown on `tab` once its tasks are read.
  pub fn overview(scenario: &str, tab: OverviewTab) -> Self {
    Self::shown(Self::reading(scenario, false), tab)
  }

  /// `scenario`'s Overview, after the run of its oldest pending tool request was damaged so that it cannot
  /// be read, as an edit outside Orchester would leave it.
  pub fn damaging_approval_run(scenario: &str) -> Self {
    let seeded = Self::prepared(scenario, false, |repo| {
      let db = rusqlite::Connection::open(repo.join(".orchester").join("orchester.db")).expect("the project database");
      let run: String = db
        .query_row(
          "SELECT run_id FROM tool_approvals WHERE state = 'pending' ORDER BY requested_at LIMIT 1",
          [],
          |row| row.get(0),
        )
        .expect("a pending tool request");
      let changed = db
        .execute("UPDATE runs SET snapshot = 'not json' WHERE id = ?1", [run])
        .expect("damage the run");
      assert_eq!(changed, 1);
    });
    Self::shown(seeded, OverviewTab::Running)
  }

  fn shown(mut seeded: Self, tab: OverviewTab) -> Self {
    seeded.go(tab);
    seeded.until("the tasks", |seeded| {
      seeded.shows("overview-footer") || seeded.shows("overview-empty")
    });
    seeded
  }

  pub fn go(&mut self, tab: OverviewTab) {
    self.window.navigation().go(self.route(tab));
    self.window.pass();
  }

  pub fn route(&self, tab: OverviewTab) -> Route {
    Route::Project(self.project, ProjectPage::Overview(tab))
  }

  /// The project's tasks as the page counts them: read from its database, without a runner.
  pub fn tasks(&self) -> ProjectTasks {
    ProjectTasks::load(&self.repo)
      .expect("the project's tasks read")
      .without_runner()
  }

  /// The texts inside `id`, icon glyphs left out; panics when nothing has the id.
  pub fn texts(&mut self, id: &str) -> Vec<String> {
    self.window.texts(id).unwrap_or_else(|| panic!("{id} is shown"))
  }

  pub fn text(&mut self, id: &str) -> String {
    self.texts(id).join(" ")
  }

  pub fn shows(&mut self, id: &str) -> bool {
    self.window.tree.get_element_by_id(id).is_some()
  }

  /// The ids under the page that start with `prefix`, in tree order.
  pub fn ids(&mut self, prefix: &str) -> Vec<String> {
    let mut ids = Vec::new();
    if let Some(root) = self.window.tree.root() {
      collect_ids(root, prefix, &mut ids);
    }
    ids
  }

  /// Where `id` is laid out: its top.
  pub fn top(&mut self, id: &str) -> f32 {
    self
      .window
      .tree
      .get_element_by_id_mut(id)
      .and_then(|element| element.bounds())
      .unwrap_or_else(|| panic!("{id} is laid out"))
      .y
  }

  /// Delivers what the window's futures finished until `done` holds.
  pub fn until(&mut self, what: &str, mut done: impl FnMut(&mut Self) -> bool) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
      self.window.tree.tick_futures();
      self.window.pass();
      if done(self) {
        return;
      }
      assert!(std::time::Instant::now() < deadline, "timed out waiting for {what}");
      std::thread::sleep(std::time::Duration::from_millis(5));
    }
  }

  /// A press with the pointer at the middle of `id`.
  pub fn click(&mut self, id: &str) {
    let bounds = self
      .window
      .tree
      .get_element_by_id_mut(id)
      .and_then(|element| element.bounds())
      .unwrap_or_else(|| panic!("{id} is laid out"));
    let (x, y) = (bounds.x + bounds.width / 2.0, bounds.y + bounds.height / 2.0);
    let tree = &mut self.window.tree;
    tree.mouse_move(x, y);
    tree.mouse_down(x, y, MouseButton::Left);
    tree.mouse_up(x, y, MouseButton::Left);
    self.window.pass();
  }
}

fn collect_ids(element: lurq::node::element::ElementRef<'_>, prefix: &str, into: &mut Vec<String>) {
  if let Some(id) = element.id().filter(|id| id.starts_with(prefix)) {
    into.push(id.to_owned());
  }
  for child in element.children() {
    collect_ids(child, prefix, into);
  }
}
