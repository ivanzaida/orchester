//! A window on a seeded Kontur, its fake `gh` answering as its mode file says, driven by element id.

use std::{fs, num::NonZeroU64, path::PathBuf};

use orchester_app::routes::{ProjectPage, Route};
use orchester_core::{id::ProjectId, time::Timestamp};
use orchester_workspace::pull_requests::list::PullRequestTab;

use crate::{
  kontur::{self, SeedRequest, scenarios},
  support::{Data, Window},
};

pub struct Seeded {
  pub window: Window,
  pub project: ProjectId,
  pub repo: PathBuf,
}

impl Seeded {
  pub fn new(scenario: &str) -> Self {
    let data = Data::new();
    let seeded = kontur::seed(&SeedRequest {
      data_dir: data.dir.path().to_path_buf(),
      repo: data.dir.path().join(format!("repo-{scenario}")),
      scenario: scenarios::named(scenario).expect("a known scenario"),
      fake_codex: None,
      now: Timestamp::now(),
    })
    .expect("the scenario seeds");
    let launch = data.launch();
    let window = Window::of(data, launch);
    Self {
      window,
      project: seeded.project,
      repo: seeded.repo,
    }
  }

  /// The `pull-requests` scenario with its fake `gh` answering as `mode` says.
  pub fn answering(mode: &str) -> Self {
    let seeded = Self::new("pull-requests");
    seeded.mode(mode);
    seeded
  }

  /// Makes the fake `gh` answer as `mode` says from its next command on.
  pub fn mode(&self, mode: &str) {
    fs::write(self.bin().join("gh-mode.txt"), format!("{mode}\n")).expect("the mode is written");
  }

  pub fn bin(&self) -> PathBuf {
    self.repo.join(".orchester").join("qa-bin")
  }

  /// The page on `tab`, once a sync finished.
  pub fn open(&mut self, tab: PullRequestTab) {
    self
      .window
      .navigation()
      .go(Route::Project(self.project, ProjectPage::PullRequests(tab)));
    self.settled();
  }

  /// Waits until no sync runs: the status no longer says so.
  pub fn settled(&mut self) {
    self.window.until("the sync to end", |window| {
      window
        .texts("pull-requests-sync-status")
        .is_some_and(|texts| !texts.iter().any(|text| text == "Syncing…"))
    });
  }

  pub fn text(&mut self, id: &str) -> String {
    self.window.text(id)
  }

  /// The command the failed sync's notice says to run, as it is copied.
  pub fn command(&mut self) -> String {
    self
      .window
      .texts("pull-requests-command")
      .expect("the notice names a command")
      .into_iter()
      .filter(|text| text != "$" && text != "Copy")
      .collect::<String>()
      .trim()
      .to_owned()
  }

  /// Where element `id` was laid out: its left, top, width and height.
  pub fn bounds(&mut self, id: &str) -> (f32, f32, f32, f32) {
    let bounds = self
      .window
      .tree
      .get_element_by_id_mut(id)
      .and_then(|element| element.bounds())
      .unwrap_or_else(|| panic!("{id} is laid out"));
    (bounds.x, bounds.y, bounds.width, bounds.height)
  }

  pub fn shows(&mut self, id: &str) -> bool {
    self.window.tree.get_element_by_id(id).is_some()
  }

  /// The numbers of the rows shown, top first.
  pub fn rows(&mut self) -> Vec<u64> {
    let mut rows: Vec<(f32, u64)> = Vec::new();
    for number in [125, 127, 128, 131, 133, 134, 136, 137, 139, 142] {
      let id = format!("pull-request-row-{number}");
      if let Some(bounds) = self
        .window
        .tree
        .get_element_by_id_mut(&id)
        .and_then(|element| element.bounds())
      {
        rows.push((bounds.y, number));
      }
    }
    rows.sort_by(|a, b| a.0.total_cmp(&b.0));
    rows.into_iter().map(|(_, number)| number).collect()
  }

  /// The key of the task seeded with `title`.
  pub fn key(&self, title: &str) -> String {
    use orchester_core::store::TaskStore;
    orchester_store::ProjectDb::open(&self.repo)
      .expect("the project's database opens")
      .tasks()
      .expect("tasks")
      .into_iter()
      .find(|task| task.details().title.as_str() == title)
      .unwrap_or_else(|| panic!("a task titled {title}"))
      .key()
      .to_string()
  }
}

/// Pull request `number` as a route names it.
pub fn nonzero(number: u64) -> NonZeroU64 {
  NonZeroU64::new(number).expect("a pull request number")
}
