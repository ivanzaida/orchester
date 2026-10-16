//! A window on a seeded project's Usage page, tall enough that the whole page is laid out, and what the tests
//! read off it.

use std::path::Path;

use lurq::app::events::MouseButton;
use orchester_app::routes::{ProjectPage, Route};
use orchester_core::{id::ProjectId, time::Timestamp};
use orchester_workspace::usage::UsageQuery;

use crate::{
  kontur::{self, SeedRequest, scenarios},
  support::{Data, Window},
};

/// Room for the whole page.
const TALL: (u32, u32) = (1440, 2200);

pub struct Seeded {
  pub window: Window,
  pub project: ProjectId,
}

impl Seeded {
  /// `scenario` seeded into a new data folder, changed by `prepare` in its repository, and its window, its
  /// reads held while `held`.
  pub fn prepared(scenario: &str, held: bool, prepare: impl FnOnce(&Path)) -> Self {
    Self::prepared_at(scenario, Timestamp::now(), held, prepare)
  }

  /// `prepared`, with the scenario seeded as if it were `now`.
  pub fn prepared_at(scenario: &str, now: Timestamp, held: bool, prepare: impl FnOnce(&Path)) -> Self {
    let data = Data::new();
    let request = SeedRequest {
      data_dir: data.dir.path().to_path_buf(),
      repo: data.dir.path().join(format!("repo-{scenario}")),
      scenario: scenarios::named(scenario).expect("a known scenario"),
      fake_codex: None,
      now,
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
    }
  }

  /// `scenario`'s Usage page on `query`, once it has read the usage.
  pub fn usage(scenario: &str, query: UsageQuery) -> Self {
    Self::usage_at(scenario, Timestamp::now(), query)
  }

  /// `usage`, with the scenario seeded as if it were `now`.
  pub fn usage_at(scenario: &str, now: Timestamp, query: UsageQuery) -> Self {
    let mut seeded = Self::prepared_at(scenario, now, false, |_| {});
    seeded.go(query);
    seeded.until("the usage", |seeded| {
      seeded.shows("usage-tokens-chart") || seeded.shows("usage-no-runs") || seeded.shows("usage-unmatched")
    });
    seeded
  }

  pub fn go(&mut self, query: UsageQuery) {
    self.window.navigation().go(self.route(query));
    self.window.pass();
  }

  pub fn route(&self, query: UsageQuery) -> Route {
    Route::Project(self.project, ProjectPage::Usage(Box::new(query)))
  }

  /// The query of the page the window shows.
  pub fn query(&self) -> UsageQuery {
    match self.window.navigation().route_now() {
      Route::Project(_, ProjectPage::Usage(query)) => *query,
      other => panic!("the window shows the Usage page, not {other:?}"),
    }
  }

  pub fn text(&mut self, id: &str) -> String {
    self
      .window
      .texts(id)
      .unwrap_or_else(|| panic!("{id} is shown"))
      .join(" ")
  }

  pub fn shows(&mut self, id: &str) -> bool {
    self.window.tree.get_element_by_id(id).is_some()
  }

  /// Delivers what the window's futures finished until `done` holds.
  pub fn until(&mut self, what: &str, mut done: impl FnMut(&mut Self) -> bool) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
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

  /// Where `id` is laid out, in logical pixels: (x, y, width, height).
  pub fn bounds(&mut self, id: &str) -> (f32, f32, f32, f32) {
    let bounds = self
      .window
      .tree
      .get_element_by_id_mut(id)
      .and_then(|element| element.bounds())
      .unwrap_or_else(|| panic!("{id} is laid out"));
    (bounds.x, bounds.y, bounds.width, bounds.height)
  }

  /// A press with the pointer at the middle of `id`, and what the window does with it.
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
    self.window.tree.tick_futures();
    self.window.pass();
  }
}
