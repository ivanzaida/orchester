//! The `review-changed` scenario's review page in the app's window, shared by the tests of `review_since` and
//! `review_since_approve`: the page, how it opens and what it reads.

use std::{
  path::{Path, PathBuf},
  process::Command,
  time::Duration,
};

use orchester_app::routes::{ProjectPage, Route};
use orchester_core::{id::ProjectId, project::TaskKey, time::Timestamp};

use crate::{
  kontur::{self, SeedRequest, scenarios},
  support::{Data, Window, stand_ins::StandIns},
};

/// How long reading a run's diffs with git may take on a loaded machine.
pub const GIT_WORK: Duration = Duration::from_secs(60);
/// A file of the run's whole diff that the two commits after the review do not touch: its rows are laid out where they
/// show, so its words tell the whole diff from the diff since the review.
/// The first file of the run's whole diff.
pub const MIGRATION: &str = "migrations/0042_delivery_attempts.sql";
pub const SENDER: &str = "src/webhooks/sender.rs";

pub struct Page {
  pub window: Window,
  _stand_ins: StandIns,
  pub project: ProjectId,
  key: TaskKey,
  pub repo: PathBuf,
}

impl Page {
  /// The seed's `scenario` in a window whose project has no runner; `before` changes the repository first.
  pub fn of(scenario: &str, before: impl FnOnce(&Path)) -> Self {
    let data = Data::new();
    let repo = data.dir.path().join(format!("repo-{scenario}"));
    let request = SeedRequest {
      data_dir: data.dir.path().to_path_buf(),
      repo: repo.clone(),
      scenario: scenarios::named(scenario).expect("a known scenario"),
      fake_codex: None,
      now: Timestamp::now(),
    };
    let seeded = kontur::seed(&request).expect("the scenario seeds");
    before(&repo);
    let launch = data.launch_with(data.no_runner());
    let (mut window, stand_ins) = Window::launched_with_stand_ins(data, launch);
    window.tree.resize(1440, 1400);
    let task = seeded.tasks.iter().find(|task| task.title == "Sign webhook payloads");
    Self {
      window,
      _stand_ins: stand_ins,
      project: seeded.project,
      repo,
      key: task
        .expect("the scenario seeds the task")
        .key
        .parse()
        .expect("a task key"),
    }
  }

  /// The `review-changed` scenario.
  pub fn new(before: impl FnOnce(&Path)) -> Self {
    Self::of("review-changed", before)
  }

  /// Opens the task's review at its route.
  pub fn open(&mut self) {
    let route = Route::Project(self.project, ProjectPage::TaskReview(self.key.clone()));
    self.window.navigation().go(route);
    self.window.until_within(GIT_WORK, "the review page", |window| {
      window.texts("review-status-text").is_some() && window.texts("review-columns").is_some()
    });
  }

  /// Waits for the diff column to show a diff whose words satisfy `done`.
  pub fn until_diff(&mut self, what: &str, done: impl Fn(&str) -> bool) {
    self.window.until_within(GIT_WORK, what, |window| {
      window.texts("review-diff").is_some() && done(&window.text("review-diff"))
    });
  }

  pub fn status(&mut self) -> String {
    self.window.text("review-status-meta")
  }
}

pub fn git(repo: &Path, args: &[&str]) -> String {
  let output = Command::new("git")
    .arg("-C")
    .arg(repo)
    .args(args)
    .output()
    .expect("git runs");
  assert!(
    output.status.success(),
    "git {args:?}: {}",
    String::from_utf8_lossy(&output.stderr)
  );
  String::from_utf8(output.stdout)
    .expect("git prints text")
    .trim()
    .to_owned()
}
