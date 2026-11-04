//! The team scenarios, each Kontur with the catalogue and no task (`empty`) and a marker beside the application
//! database that names its state: the stand-in team server has to answer while the window is open, so what joins it is
//! the QA app's own (`examples/qa_app/team`), and so are `team-clone`'s clones. `team-offer` also has Ledger, a local
//! project, so All projects shows *Local* beside *Shared · Acme* (09 `i5G19A`); `team-outage-running` has two tasks
//! running on this computer.

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
use crate::kontur::{
  Kontur, SeedError, files, git, io_error as io, name,
  shapes::{self, Shape},
};

/// The tasks running on this computer in `team-outage-running`, as `On this computer` (`XTBXg`) lists them: number,
/// title, the step at work and how many minutes ago the run started.
const RUNNING: [(u32, &str, &str, i64); 2] = [
  (12, "Add retry to webhook sender", "implement", 40),
  (21, "Replay a failed delivery", "plan", 12),
];

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

pub fn download_progress(kontur: &mut Kontur) -> Result<(), SeedError> {
  mark(kontur, State::DownloadProgress)?;
  origin(kontur)
}

pub fn download_stopped(kontur: &mut Kontur) -> Result<(), SeedError> {
  mark(kontur, State::DownloadStopped)?;
  origin(kontur)
}

/// With three folders beside the database to link the team's Kontur to, in `checkouts`: `kontur` matches its repository
/// hint (06 `go2aM`), `kontur-fork` has another `origin` (13b `tDFeg`), `plain` is no repository (13 `sN8Pa`).
pub fn shared_projects(kontur: &mut Kontur) -> Result<(), SeedError> {
  mark(kontur, State::SharedProjects)?;
  let checkouts = kontur.app_database.with_file_name("checkouts");
  for (name, origin) in [
    ("kontur", "https://github.com/acme/kontur.git"),
    ("kontur-fork", "https://github.com/maria/kontur-fork.git"),
  ] {
    let folder = checkouts.join(name);
    fs::create_dir_all(&folder).map_err(io(&folder))?;
    git::init(&folder, &files::initial())?;
    git::add_origin(&folder, origin)?;
  }
  let plain = checkouts.join("plain");
  fs::create_dir_all(&plain).map_err(io(&plain))
}

pub fn shared_none(kontur: &mut Kontur) -> Result<(), SeedError> {
  mark(kontur, State::NoSharedProjects)
}

/// Acme's projects, each cloned from *Link a folder* by the QA app's stand-in console before the window shows
/// (`examples/qa_app/team_clone.rs`), which makes the folder they go in.
pub fn clones(kontur: &mut Kontur) -> Result<(), SeedError> {
  mark(kontur, State::Clones)
}

/// Kontur linked and joined as a member, its tasks read from the team server (`01 · Tasks · member` `QdL7N`).
pub fn member(kontur: &mut Kontur) -> Result<(), SeedError> {
  mark(kontur, State::Member)
}

/// As [`member`], as an Admin of Kontur (`07 · Roles` `BLTsP`).
pub fn admin(kontur: &mut Kontur) -> Result<(), SeedError> {
  mark(kontur, State::Admin)
}

/// As [`member`], as the server's admin, no member of Kontur (`07 · Roles` `BLTsP`).
pub fn server_admin(kontur: &mut Kontur) -> Result<(), SeedError> {
  mark(kontur, State::ServerAdmin)
}

/// As [`member`], with the team server's reads unavailable (`06 · Tasks · team data unavailable` `GR3RS`).
pub fn outage(kontur: &mut Kontur) -> Result<(), SeedError> {
  mark(kontur, State::Outage)
}

/// As [`outage`], with KON-12 running Implement and KON-21 running Plan on this computer (`On this computer` `XTBXg`):
/// the runs are this computer's, so they show while the team's tasks cannot be read.
pub fn outage_running(kontur: &mut Kontur) -> Result<(), SeedError> {
  let catalogue = kontur.catalogue()?;
  let pipeline = &catalogue.feature_delivery;
  for (number, title, step, minutes) in RUNNING {
    // The store numbers tasks in order and has no API to number one: the seed moves its counter on, as in a project
    // whose earlier tasks are the server's.
    rusqlite::Connection::open(kontur.database())?
      .execute("UPDATE counters SET next = ?1 WHERE name = 'task'", [number])?;
    let (created, started) = (kontur.ago(minutes + 30), kontur.ago(minutes));
    let mut task = kontur.task((title, title, &[title]), Some(pipeline), created)?;
    shapes::apply(kontur, &mut task, Some(pipeline), Shape::Running(step), started)?;
  }
  write_marker(kontur, State::OutageRunning)
}

/// As [`member`]; the QA app has the server stop answering once the tasks were shown.
pub fn outage_later(kontur: &mut Kontur) -> Result<(), SeedError> {
  mark(kontur, State::OutageLater)
}

/// Kontur's checkout has the team's repository as its `origin`, which the QA app's server names for its project.
fn origin(kontur: &Kontur) -> Result<(), SeedError> {
  git::add_origin(&kontur.repo, "https://github.com/acme/kontur.git")?;
  Ok(())
}

/// The catalogue and the marker naming `state`.
fn mark(kontur: &mut Kontur, state: State) -> Result<(), SeedError> {
  kontur.catalogue()?;
  write_marker(kontur, state)
}

fn write_marker(kontur: &Kontur, state: State) -> Result<(), SeedError> {
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
  // The signed-in account's project, or this computer's while nobody is signed in.
  let owner = apps.cloud_account()?.map(|account| account.account);
  // Opened a day before Kontur, which stays the project the app opens.
  apps.register_project(&project, &folder, kontur.ago(24 * 60), owner.as_ref())?;
  Ok(())
}
