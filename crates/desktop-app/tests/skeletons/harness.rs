//! A window on the seeded project whose reads are held from the start, and what the tests read off it.

use std::time::{Duration, Instant};

use lurq::core::ElementRect;
use orchester_app::{
  project::hold::ReadHold,
  routes::{ProjectPage, Route},
  theme::SKELETON_DELAY,
};
use orchester_core::{id::ProjectId, project::TaskKey, time::Timestamp};

use crate::{
  kontur::{self, SeedRequest, scenarios},
  support::{Data, Window},
};

pub struct Held {
  pub window: Window,
  pub project: ProjectId,
  /// A seeded task's key, for its page.
  pub task: TaskKey,
  pub hold: ReadHold,
}

impl Held {
  /// The `tasks` scenario seeded into a new data folder, its reads held before the window shows anything.
  pub fn new() -> Self {
    let data = Data::new();
    let request = SeedRequest {
      data_dir: data.dir.path().to_path_buf(),
      repo: data.dir.path().join("repo-tasks"),
      scenario: scenarios::named("tasks").expect("a known scenario"),
      fake_codex: None,
      now: Timestamp::now(),
    };
    let seeded = kontur::seed(&request).expect("the scenario seeds");
    let launch = data.launch();
    let hold = launch.services().expect("the session started").hold.clone();
    hold.set(true);
    let task = seeded.tasks[0].key.parse().expect("a seeded task's key");
    Self {
      window: Window::of(data, launch),
      project: seeded.project,
      task,
      hold,
    }
  }

  pub fn go(&mut self, route: Route) {
    self.window.navigation().go(route);
  }

  pub fn page(&mut self, page: ProjectPage) {
    let project = self.project;
    self.go(Route::Project(project, page));
  }

  pub fn exists(&mut self, id: &str) -> bool {
    self.window.tree.get_element_by_id(id).is_some()
  }

  pub fn click(&mut self, id: &str) {
    self
      .window
      .tree
      .get_element_by_id_mut(id)
      .unwrap_or_else(|| panic!("{id} exists"))
      .click();
    self.window.pass();
  }

  pub fn bounds(&mut self, id: &str) -> ElementRect {
    self
      .window
      .tree
      .get_element_by_id_mut(id)
      .and_then(|element| element.bounds())
      .unwrap_or_else(|| panic!("{id} is laid out"))
  }

  /// Waits until `id` is in the window, the window's timers left as they are.
  pub fn until(&mut self, id: &str) {
    let id = id.to_owned();
    self
      .window
      .until(&id.clone(), move |window| window.tree.get_element_by_id(&id).is_some());
  }

  /// Lets the skeletons' delay pass, and lays the window out again.
  pub fn after_delay(&mut self) {
    self
      .window
      .tree
      .tick_timers_at(Instant::now() + SKELETON_DELAY + Duration::from_millis(50));
    self.window.pass();
  }

  /// The skeleton `id` is in the window with no bone, and shows its bones once the delay passed.
  pub fn skeleton(&mut self, id: &str) {
    self.until(id);
    let bones = format!("{id}-bones");
    assert!(!self.exists(&bones), "{id}: a fast reading never flashes its bones");
    self.after_delay();
    assert!(
      self.exists(&bones),
      "{id}: its bones show once the reading took a moment"
    );
  }

  /// Lets the reads go and waits until `id` is in the window.
  pub fn release(&mut self, id: &str) {
    self.hold.set(false);
    self.until(id);
  }
}
