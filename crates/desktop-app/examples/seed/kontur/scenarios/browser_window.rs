//! The Browser window's QA states (`Browser · redesign` `lcNez`, ORC-379): the `tasks` project, and beside the app's
//! database the fixture the QA app plays into the window's state (`orchester_app::test_support::browser_qa`): which
//! state, the project, and the latest runs of the tasks that have runs, KON-12's first, whose agents own the tabs.
//! The QA app opens the window by itself as an agent starting to browse does, on a page the computer's own browser
//! renders when it has one.

use std::fs;

use orchester_app::test_support::browser_qa::{BrowserState, Fixture};
use orchester_core::{
  id::RunId,
  store::{RunStore, TaskStore},
};

use super::tasks;
use crate::kontur::{Kontur, PROJECT_NAME, SeedError};

pub fn default(kontur: &mut Kontur) -> Result<(), SeedError> {
  seed(kontur, BrowserState::Default)
}

pub fn many(kontur: &mut Kontur) -> Result<(), SeedError> {
  seed(kontur, BrowserState::Many)
}

pub fn overflow(kontur: &mut Kontur) -> Result<(), SeedError> {
  seed(kontur, BrowserState::Overflow)
}

pub fn acting(kontur: &mut Kontur) -> Result<(), SeedError> {
  seed(kontur, BrowserState::Acting)
}

pub fn waiting(kontur: &mut Kontur) -> Result<(), SeedError> {
  seed(kontur, BrowserState::Waiting)
}

pub fn dialog(kontur: &mut Kontur) -> Result<(), SeedError> {
  seed(kontur, BrowserState::Dialog)
}

pub fn file_chooser(kontur: &mut Kontur) -> Result<(), SeedError> {
  seed(kontur, BrowserState::FileChooser)
}

pub fn file_chosen(kontur: &mut Kontur) -> Result<(), SeedError> {
  seed(kontur, BrowserState::FileChosen)
}

pub fn select(kontur: &mut Kontur) -> Result<(), SeedError> {
  seed(kontur, BrowserState::Select)
}

pub fn headed(kontur: &mut Kontur) -> Result<(), SeedError> {
  seed(kontur, BrowserState::Headed)
}

pub fn empty(kontur: &mut Kontur) -> Result<(), SeedError> {
  seed(kontur, BrowserState::Empty)
}

pub fn not_found(kontur: &mut Kontur) -> Result<(), SeedError> {
  seed(kontur, BrowserState::NotFound)
}

fn seed(kontur: &mut Kontur, state: BrowserState) -> Result<(), SeedError> {
  tasks::project(kontur)?;
  let first = kontur.find("KON-12")?;
  let mut tasks = kontur.db.tasks()?;
  tasks.sort_by_key(|task| task.id() != first.id());
  let mut runs: Vec<RunId> = Vec::new();
  for task in tasks {
    let latest = kontur
      .db
      .runs_of_task(task.id())?
      .into_iter()
      .filter_map(Result::ok)
      .next_back();
    runs.extend(latest.map(|run| run.id()));
  }
  let fixture = Fixture {
    state,
    project: kontur.repo.clone(),
    name: PROJECT_NAME.to_owned(),
    runs,
  };
  let path = kontur.app_database.with_file_name(Fixture::FILE);
  fs::write(&path, fixture.to_json().to_string()).map_err(|source| SeedError::Io { path, source })
}
