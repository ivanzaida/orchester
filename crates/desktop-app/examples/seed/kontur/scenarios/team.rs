//! The team scenarios, each Kontur with the catalogue and no task (`empty`) and a marker beside the application
//! database that names its state: the stand-in team server has to answer while the window is open, so what joins it is
//! the QA app's own (`examples/qa_app/team`). `team-offer` also has Ledger, a local project, so All projects shows
//! *Local* beside *Shared · Acme* (09 `i5G19A`).

#[allow(
  dead_code,
  reason = "the QA app's marker, which it reads itself; the seed only names the state it writes"
)]
#[path = "../../../qa_app/team/state.rs"]
pub mod state;

use std::fs;

use orchester_core::{
  id::ProjectId,
  project::{Project, ProjectKey},
};
use orchester_store::{AppDb, ProjectDb};

use self::state::{FILE, State};
use crate::kontur::{Kontur, SeedError, files, git, io_error as io, name};

pub fn join(kontur: &mut Kontur) -> Result<(), SeedError> {
  mark(kontur, State::Join)
}

pub fn wrong_code(kontur: &mut Kontur) -> Result<(), SeedError> {
  mark(kontur, State::WrongCode)
}

pub fn expired_code(kontur: &mut Kontur) -> Result<(), SeedError> {
  mark(kontur, State::ExpiredCode)
}

pub fn unreachable(kontur: &mut Kontur) -> Result<(), SeedError> {
  mark(kontur, State::Unreachable)
}

pub fn joined(kontur: &mut Kontur) -> Result<(), SeedError> {
  mark(kontur, State::Joined)
}

pub fn offer(kontur: &mut Kontur) -> Result<(), SeedError> {
  mark(kontur, State::Offer)?;
  ledger(kontur)
}

pub fn downloading(kontur: &mut Kontur) -> Result<(), SeedError> {
  mark(kontur, State::Downloading)
}

pub fn download_failed(kontur: &mut Kontur) -> Result<(), SeedError> {
  mark(kontur, State::DownloadFailed)
}

fn mark(kontur: &mut Kontur, state: State) -> Result<(), SeedError> {
  kontur.catalogue()?;
  let marker = kontur.app_database.with_file_name(FILE);
  fs::write(&marker, state.name()).map_err(io(&marker))
}

/// Ledger, a repository of its own beside Kontur's, registered in the app's database before Kontur.
fn ledger(kontur: &Kontur) -> Result<(), SeedError> {
  let folder = kontur.app_database.with_file_name("repo-ledger");
  fs::create_dir_all(&folder).map_err(io(&folder))?;
  git::init(&folder, &files::initial())?;
  let project = Project::new(
    ProjectId::generate(),
    ProjectKey::new("LED").expect("a valid project key"),
    name("Ledger"),
  );
  ProjectDb::create(&folder, project.clone())?;
  let mut apps = AppDb::open(&kontur.app_database)?;
  // The signed-in account's project, or the next one's to sign in.
  let owner = apps.cloud_account()?.map(|account| account.account);
  // Opened a day before Kontur, which stays the project the app opens.
  apps.register_project(&project, &folder, kontur.ago(24 * 60), owner.as_ref())?;
  Ok(())
}
