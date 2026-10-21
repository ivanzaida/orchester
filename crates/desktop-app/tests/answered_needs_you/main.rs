//! An answered *Needs you* item leaves at once (ORC-70), headless at 1440 × 900 on the QA seed's `live` scenario with
//! the real runner and the fake Codex: each place an answer is given — the Needs you panel in place, the task
//! Conversation's card, the Summary's decision bar, the composer, the Orchestrator chat's tools while the Overview
//! shows — drops the item and the count in the press's frame, for the active project and for one in another tab,
//! and the runner's record of it lets the feed read the project again at once; a refused answer brings the item back
//! with its reason where it was given ([`refusal`]). Each test prints how long the item took to leave, from the press
//! to the frame the feed no longer lists it (`ORC-70 timing`; `cargo test ... -- --nocapture` shows them). Needs `git`.

#[path = "../../examples/seed/kontur/mod.rs"]
#[allow(dead_code, reason = "the seed's helpers the other scenarios use")]
mod kontur;
mod places;
mod refusal;
#[path = "../support/mod.rs"]
mod support;

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
use orchester_workspace::{
  tasks::needs_you::{WaitingItem, WaitingKind},
  workspace::OpenProject,
};
use support::{Data, Window, runner_programs::programs};
use tempfile::TempDir;
use tokio::runtime::Runtime;

/// How long a real runner and the fake agent may take to reach a wait.
const PATIENCE: Duration = Duration::from_secs(90);

/// The window on a seeded scenario's project, and the seeded tasks: key and title.
struct Page {
  window: Window,
  project: ProjectId,
  repo: PathBuf,
  tasks: Vec<(String, String)>,
}

impl Page {
  /// The `live` scenario, its Codex connection running `orchester-fake-codex`, with the real runner.
  fn live() -> Self {
    let programs = programs();
    Self::with("live", Some((programs.runner.clone(), programs.fake_codex.clone())))
  }

  /// `scenario` seeded, the window started with `programs` (the runner and the fake Codex) when given, or with a
  /// runner program that does not exist.
  fn with(scenario: &str, programs: Option<(PathBuf, PathBuf)>) -> Self {
    let dir = TempDir::new().expect("a temporary folder");
    let repo = dir.path().join(format!("repo-{scenario}"));
    let request = kontur::SeedRequest {
      data_dir: dir.path().to_path_buf(),
      repo: repo.clone(),
      scenario: kontur::scenarios::named(scenario).expect("a known scenario"),
      fake_codex: programs.as_ref().map(|(_, codex)| codex.clone()),
      now: Timestamp::now(),
    };
    let seeded = kontur::seed(&request).expect("the scenario seeds");
    let data = Data::at(dir, Runtime::new().expect("tokio runtime"));
    let launch = match programs {
      Some((runner, _)) => data.launch_with(runner),
      None => data.launch(),
    };
    let mut window = Window::of(data, launch);
    window.until("the project's tasks to be read", |window| {
      window.texts("needs-you-pill").is_some()
    });
    let tasks = seeded.tasks.into_iter().map(|task| (task.key, task.title)).collect();
    Self {
      window,
      project: seeded.project,
      repo,
      tasks,
    }
  }

  fn key_of(&self, title: &str) -> String {
    self
      .tasks
      .iter()
      .find(|(_, seeded)| seeded == title)
      .unwrap_or_else(|| panic!("a task {title}"))
      .0
      .clone()
  }

  fn open_project(&self) -> OpenProject {
    self
      .window
      .projects()
      .state_now(self.project)
      .and_then(|state| state.open().cloned())
      .expect("the project is open")
  }

  /// Goes to `tab` of the task titled `title`.
  fn go(&mut self, title: &str, tab: TaskTab) {
    let key = self.key_of(title);
    let page = ProjectPage::Task(key.parse().expect("a task key"), TaskView::of(tab));
    self.window.navigation().go(Route::Project(self.project, page));
  }

  /// Runs the task titled `title` from its Summary, once its runner is there.
  fn start(&mut self, title: &str) {
    self.go(title, TaskTab::Summary);
    self.wait("the runner", |window| {
      window
        .tree
        .get_element_by_id("task-run-icon")
        .and_then(|icon| icon.text_content().map(str::to_owned))
        .is_some_and(|glyph| glyph != Icon::Lock.glyph().to_string())
    });
    self.click("task-run");
  }

  /// What waits in the project, as every *Needs you* reads it; `None` while its tasks are not read.
  fn waiting(&self) -> Option<Vec<WaitingItem>> {
    let feed = self.window.projects().feed(self.project);
    feed.tasks.get_untracked().ready().map(|tasks| tasks.waiting().to_vec())
  }

  fn count(&self) -> usize {
    self.waiting().map_or(0, |waiting| waiting.len())
  }

  /// Runs the window until an item `of` waits; that item.
  fn wait_for(&mut self, what: &str, of: impl Fn(&WaitingItem) -> bool) -> WaitingItem {
    let mut found = None;
    let deadline = Instant::now() + PATIENCE;
    while found.is_none() {
      assert!(Instant::now() < deadline, "timed out waiting for {what}");
      self.window.tree.tick_futures();
      self.window.pass();
      found = self.waiting().and_then(|waiting| waiting.into_iter().find(|item| of(item)));
      thread::sleep(Duration::from_millis(20));
    }
    found.expect("found above")
  }

  /// As the window's `until`, for work a real runner and agent do.
  fn wait(&mut self, what: &str, mut done: impl FnMut(&mut Window) -> bool) {
    let deadline = Instant::now() + PATIENCE;
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

  /// [`Self::wait`], for what the page itself tells.
  fn wait_until(&mut self, what: &str, mut done: impl FnMut(&Self) -> bool) {
    let deadline = Instant::now() + PATIENCE;
    loop {
      self.window.tree.tick_futures();
      self.window.pass();
      if done(self) {
        return;
      }
      assert!(Instant::now() < deadline, "timed out waiting for {what}");
      thread::sleep(Duration::from_millis(20));
    }
  }

  /// Scrolls `id` into view and clicks its middle; the window lays out once, delivering nothing its futures did.
  fn click(&mut self, id: &str) {
    if let Some(mut element) = self.window.tree.get_element_by_id_mut(id) {
      element.scroll_into_view();
    }
    self.window.pass();
    let bounds = self
      .window
      .tree
      .get_element_by_id_mut(id)
      .and_then(|element| element.bounds())
      .unwrap_or_else(|| panic!("{id} is laid out"));
    let scale = self.window.tree.scale_factor();
    let (x, y) = (
      (bounds.x + bounds.width / 2.0) * scale,
      (bounds.y + bounds.height / 2.0) * scale,
    );
    self.window.tree.mouse_move(x, y);
    self.window.tree.mouse_down(x, y, MouseButton::Left);
    self.window.tree.mouse_up(x, y, MouseButton::Left);
  }

  /// Every element id under `id` that starts with `prefix`, shortest first.
  fn ids(&mut self, id: &str, prefix: &str) -> Vec<String> {
    let mut found = Vec::new();
    if let Some(element) = self.window.tree.get_element_by_id(id) {
      ids_in(element, prefix, &mut found);
    }
    found.sort_by_key(String::len);
    found
  }

  /// Whether an item of `item`'s run and kind still waits.
  fn lists(&self, item: &WaitingItem) -> bool {
    self
      .waiting()
      .is_some_and(|waiting| waiting.iter().any(|waits| same(waits, item)))
  }

  /// The count the title bar's pill shows: its words start with it.
  fn pill(&mut self) -> usize {
    let words = self.window.text("needs-you-pill");
    words
      .split_whitespace()
      .next()
      .and_then(|count| count.parse().ok())
      .unwrap_or_else(|| panic!("the pill counts: {words}"))
  }

  /// Answers `item` with `press`, then measures: whether it left and the count and the pill went down by one in the
  /// press's own frame (two layouts, nothing a future did delivered), and how long, from the press, until the feed
  /// lists it no more. Prints the time as `label`'s.
  fn answer(&mut self, label: &str, item: &WaitingItem, press: impl FnOnce(&mut Self)) -> Answered {
    let before = self.count();
    let pressed = Instant::now();
    press(self);
    self.window.pass();
    self.window.pass();
    let at_once = !self.lists(item) && self.count() + 1 == before && self.pill() + 1 == before;
    let deadline = pressed + PATIENCE;
    while self.lists(item) {
      assert!(Instant::now() < deadline, "timed out waiting for {label} to leave");
      self.window.tree.tick_futures();
      self.window.pass();
      thread::sleep(Duration::from_millis(2));
    }
    let took = pressed.elapsed();
    eprintln!(
      "ORC-70 timing {label}: {} ms to leave (in the press's frame: {at_once})",
      took.as_millis()
    );
    Answered { at_once }
  }

  /// Makes the project another tab's: a tab at All projects opens and becomes the active one.
  fn away(&mut self) {
    self.window.navigation().open_tab();
    self.window.until("the other tab", |window| {
      window.navigation().route_now() == Route::AllProjects
    });
  }

  /// Stops the runner and its agents, so none outlives the test.
  fn end(self) {
    let services = self.window.launch.services().expect("the session started").clone();
    services.runners.terminate();
  }
}

/// What [`Page::answer`] measured.
struct Answered {
  at_once: bool,
}

/// One waiting object, however it reads now: the same run waits for the same kind of thing.
fn same(one: &WaitingItem, other: &WaitingItem) -> bool {
  one.run == other.run && std::mem::discriminant(&one.kind) == std::mem::discriminant(&other.kind)
}

fn ids_in(element: ElementRef<'_>, prefix: &str, into: &mut Vec<String>) {
  if let Some(id) = element.id().filter(|id| id.starts_with(prefix)) {
    into.push(id.to_owned());
  }
  for child in element.children() {
    ids_in(child, prefix, into);
  }
}

fn is_approval(item: &WaitingItem) -> bool {
  matches!(item.kind, WaitingKind::Approval(_))
}

fn is_choice(item: &WaitingItem) -> bool {
  matches!(item.kind, WaitingKind::Choice(_))
}

fn is_plan(item: &WaitingItem) -> bool {
  matches!(item.kind, WaitingKind::Plan(_))
}

fn is_decision(item: &WaitingItem) -> bool {
  matches!(item.kind, WaitingKind::Decision(_))
}
