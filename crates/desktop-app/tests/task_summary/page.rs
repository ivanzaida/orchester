//! The window on a seeded scenario's project, opened on a task's Summary, and what the tests read and press there.

use std::{
  path::PathBuf,
  thread,
  time::{Duration, Instant},
};

use lurq::{app::events::MouseButton, node::element::ElementRef};
use orchester_app::{
  routes::{ProjectPage, Route, TaskTab, TaskView},
  ui::icon::Icon,
};
use orchester_core::{id::ProjectId, time::Timestamp};
use tempfile::TempDir;
use tokio::runtime::Runtime;

use crate::{
  kontur,
  support::{Data, Window, runner_programs::programs},
};

/// The window on a seeded scenario's project, and the seeded tasks: key, title and state.
pub struct Page {
  pub window: Window,
  project: ProjectId,
  tasks: Vec<(String, String, &'static str)>,
}

impl Page {
  pub fn seeded(scenario: &str) -> Self {
    Self::with(scenario, None)
  }

  /// The `live` scenario, its Codex connection running `orchester-fake-codex`, with the real runner: the test
  /// builds both itself (`support::runner_programs`).
  pub fn live() -> Self {
    let programs = programs();
    Self::with("live", Some((programs.runner.clone(), programs.fake_codex.clone())))
  }

  /// `scenario` seeded, the window started with `programs` (the runner and the fake Codex) when given, or
  /// a runner program that does not exist.
  fn with(scenario: &str, programs: Option<(PathBuf, PathBuf)>) -> Self {
    let dir = TempDir::new().expect("a temporary folder");
    let request = kontur::SeedRequest {
      data_dir: dir.path().to_path_buf(),
      repo: dir.path().join(format!("repo-{scenario}")),
      scenario: kontur::scenarios::named(scenario).expect("a known scenario"),
      fake_codex: programs.as_ref().map(|(_, codex)| codex.clone()),
      now: Timestamp::now(),
    };
    let seeded = kontur::seed(&request).expect("the scenario seeds");
    let data = Data::at(dir, Runtime::new().expect("tokio runtime"));
    assert_eq!(
      seeded.app_database,
      data.database(),
      "the seed registers in the window's folder"
    );
    let launch = match programs {
      Some((runner, _)) => data.launch_with(runner),
      None => data.launch(),
    };
    let window = Window::of(data, launch);
    let tasks = seeded
      .tasks
      .into_iter()
      .map(|task| (task.key, task.title, task.state))
      .collect();
    Self {
      window,
      project: seeded.project,
      tasks,
    }
  }

  /// Opens the Summary of the task left in `state`, once it shows; its key.
  pub fn open(&mut self, state: &str) -> String {
    let title = self
      .tasks
      .iter()
      .find(|(_, _, seeded)| *seeded == state)
      .unwrap_or_else(|| panic!("a task {state}"))
      .1
      .clone();
    self.open_titled(&title)
  }

  /// Opens the Summary of the task titled `title`, once it shows; its key.
  pub fn open_titled(&mut self, title: &str) -> String {
    let key = self.key_of(title);
    let page = ProjectPage::Task(key.parse().expect("a task key"), TaskView::of(TaskTab::Summary));
    self.window.navigation().go(Route::Project(self.project, page));
    let title = title.to_owned();
    self.window.until(&format!("the Summary of {key}"), |window| {
      window
        .texts("task-title-row")
        .is_some_and(|texts| texts.contains(&title))
        && (window.texts("task-event").is_some()
          || window.texts("task-notice-title").is_some()
          || window.texts("task-summary-empty").is_some())
    });
    key
  }

  /// The key of the task titled `title`.
  pub fn key_of(&self, title: &str) -> String {
    self
      .tasks
      .iter()
      .find(|(_, seeded, _)| seeded == title)
      .unwrap_or_else(|| panic!("a task {title}"))
      .0
      .clone()
  }

  /// As the window's `until`, for work a real runner and agent do: up to a minute.
  pub fn wait(&mut self, what: &str, mut done: impl FnMut(&mut Window) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
      self.window.tree.tick_futures();
      self.window.pass();
      if done(&mut self.window) {
        return;
      }
      assert!(Instant::now() < deadline, "timed out waiting for {what}");
      thread::sleep(Duration::from_millis(20));
    }
  }

  pub fn text(&mut self, id: &str) -> String {
    self
      .window
      .texts(id)
      .unwrap_or_else(|| panic!("{id} is shown"))
      .join(" ")
  }

  pub fn has(&mut self, id: &str) -> bool {
    self.window.tree.get_element_by_id(id).is_some()
  }

  /// Every element id under `id` that starts with `prefix`.
  pub fn ids(&mut self, id: &str, prefix: &str) -> Vec<String> {
    let mut found = Vec::new();
    if let Some(element) = self.window.tree.get_element_by_id(id) {
      ids_in(element, prefix, &mut found);
    }
    found
  }

  /// Clicks the middle of the element `id`.
  pub fn click(&mut self, id: &str) {
    let bounds = self
      .window
      .tree
      .get_element_by_id_mut(id)
      .and_then(|element| element.bounds())
      .unwrap_or_else(|| panic!("{id} is laid out"));
    let (x, y) = (bounds.x + bounds.width / 2.0, bounds.y + bounds.height / 2.0);
    self.window.tree.mouse_move(x, y);
    self.window.tree.mouse_down(x, y, MouseButton::Left);
    self.window.tree.mouse_up(x, y, MouseButton::Left);
    self.window.pass();
  }

  /// Whether the button `id` shows the lock of an unavailable action in place of its icon.
  pub fn locked(&mut self, id: &str) -> bool {
    self
      .window
      .tree
      .get_element_by_id(&format!("{id}-icon"))
      .and_then(|icon| icon.text_content().map(str::to_owned))
      .is_some_and(|glyph| glyph == Icon::Lock.glyph().to_string())
  }
}

fn ids_in(element: ElementRef<'_>, prefix: &str, into: &mut Vec<String>) {
  let part = |id: &str| id.ends_with("-icon") || id.ends_with("-label");
  if let Some(id) = element.id().filter(|id| id.starts_with(prefix) && !part(id)) {
    into.push(id.to_owned());
  }
  for child in element.children() {
    ids_in(child, prefix, into);
  }
}
