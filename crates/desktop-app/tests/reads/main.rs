//! What a change costs in reads of project data, counted from the app's read log (`app_log::READ_LOG`) on the
//! QA seed's projects, headless: a screen reads again only what the change touched, once. The window's runner
//! program does not exist; the runner's reports are counted as it would count them
//! (`Runners::count_change_for_test`). Needs `git`.

#[allow(
  dead_code,
  reason = "the seed's report and its database path are the QA tool's; these tests read its tasks"
)]
#[path = "../../examples/seed/kontur/mod.rs"]
mod kontur;
#[path = "../support/mod.rs"]
mod support;
mod screens;
mod transcript;

use std::{
  collections::BTreeMap,
  path::{Path, PathBuf},
  thread,
  time::{Duration, Instant},
};

use kontur::{SeedRequest, Seeded, scenarios};
use orchester_app::routes::{ProjectPage, Route};
use orchester_core::{
  id::IdempotencyKey,
  name::Name,
  task::{TaskDetails, TaskLimits},
  time::Timestamp,
};
use orchester_workspace::tasks::data::{SaveTarget, save_draft_in};
use support::{Data, Window, reads::Reads};

/// A read that starts later than this after the last one belongs to no change of the test.
const QUIET: Duration = Duration::from_millis(400);

/// A window on `scenario`, seeded in its own data folder, its reads counted from the start.
struct Counted {
  /// First: it counts on the test's thread while the window lives.
  reads: Reads,
  window: Window,
  seeded: Seeded,
}

impl Counted {
  fn open(scenario: &str) -> Self {
    let reads = Reads::count();
    let data = Data::new();
    let request = SeedRequest {
      data_dir: data.dir.path().to_path_buf(),
      repo: data.dir.path().join(format!("repo-{scenario}")),
      scenario: scenarios::named(scenario).expect("a known scenario"),
      fake_codex: None,
      now: Timestamp::now(),
    };
    let seeded = kontur::seed(&request).expect("the scenario seeds");
    let launch = data.launch();
    Self {
      reads,
      window: Window::of(data, launch),
      seeded,
    }
  }

  /// Shows `page` of the seeded project until `shown` is laid out and its reads are done; counts from none.
  fn show(&mut self, page: ProjectPage, shown: &str) {
    self.window.navigation().go(Route::Project(self.seeded.project, page));
    self
      .window
      .until(shown, |window| window.tree.get_element_by_id(shown).is_some());
    self.settle();
  }

  /// Delivers what the window's futures finish until no read started for a while; the reads counted so far,
  /// by what they read, and counts again from none.
  fn settle(&mut self) -> BTreeMap<String, usize> {
    let deadline = Instant::now() + Duration::from_secs(15);
    let mut last = (self.reads.total(), Instant::now());
    while last.1.elapsed() < QUIET {
      assert!(Instant::now() < deadline, "the window kept reading");
      self.window.tree.tick_futures();
      self.window.pass();
      thread::sleep(Duration::from_millis(5));
      let total = self.reads.total();
      if total != last.0 {
        last = (total, Instant::now());
      }
    }
    self.reads.take()
  }

  /// Where the seeded project is, as its runner and feed name it.
  fn root(&self) -> PathBuf {
    self
      .window
      .projects()
      .state(self.seeded.project)
      .and_then(|state| state.open().map(|open| open.repo_root.clone()))
      .expect("the project is open")
  }

  /// Counts a report of the project's runner: a run changed state (`true`) or a transcript grew.
  fn report(&self, state: bool) {
    let services = self.window.launch.services().expect("the session started");
    services.runners.count_change_for_test(&self.root(), state);
  }
}

/// Saves a new draft task in the project at `repo`, as New task does.
fn draft(repo: &Path) {
  let details = TaskDetails {
    title: Name::new("A new draft").expect("a title"),
    brief: String::new(),
    acceptance_criteria: Vec::new(),
    pipeline: None,
    limits: TaskLimits::NONE,
  };
  save_draft_in(repo, SaveTarget::New(IdempotencyKey::generate()), &details).expect("the draft is saved");
}
