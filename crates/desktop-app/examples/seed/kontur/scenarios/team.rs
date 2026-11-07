//! The team scenarios, each Kontur with the catalogue and no task (`empty`) and a marker beside the application
//! database that names its state: the stand-in team server has to answer while the window is open, so what joins it is
//! the QA app's own (`examples/qa_app/team`), and so are `team-clone`'s clones. `team-offer` also has Ledger, a local
//! project, so All projects shows *Local* beside *Shared · Acme* (09 `i5G19A`); `team-outage-running`,
//! `team-member-running` and `team-module-outage` have two tasks running on this computer, and in `team-outage-running`
//! their work reports wait in the outbox to be sent.

#[allow(
  dead_code,
  reason = "the QA app's marker, which it reads itself; the seed only names the state it writes"
)]
#[path = "../../../qa_app/team/state.rs"]
pub mod state;

use std::fs;

use orchester_core::{
  id::{ClaimId, OperationId, ProjectId, WorkEventId, WorkId},
  project::{Project, ProjectKey},
  task::Task,
  time::Timestamp,
};
use orchester_store::{AppDb, ProjectDb};
use rusqlite::{Connection, params};

use self::state::{FILE, State};
use crate::kontur::{
  Kontur, SeedError, files, git, io_error as io, name,
  shapes::{self, Shape},
};

/// The tasks running on this computer in `team-outage-running`, as `On this computer` (`XTBXg`) lists them: number,
/// title, the step at work, how many minutes ago the run started, the run's number and how many of its work reports
/// wait to be sent.
const RUNNING: [(u32, &str, &str, i64, u32, u32); 2] = [
  (12, "Add retry to webhook sender", "implement", 40, 8, 3),
  (21, "Replay a failed delivery", "plan", 12, 2, 1),
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
  running(kontur, State::OutageRunning)
}

/// As [`member`]; the QA app has the server stop answering once the tasks were shown.
pub fn outage_later(kontur: &mut Kontur) -> Result<(), SeedError> {
  mark(kontur, State::OutageLater)
}

/// As [`member`], with KON-12 and KON-21 running on this computer: the QA app's server names them by their ids here, so
/// KON-12's page joins its run with the server's task, claimed by the person (`01c` `Tb55G`).
pub fn member_running(kontur: &mut Kontur) -> Result<(), SeedError> {
  running(kontur, State::MemberRunning)
}

/// As [`member_running`]; the QA app has the server stop answering once a module's page read its module (`06d`
/// `B7Mbe`).
pub fn module_outage(kontur: &mut Kontur) -> Result<(), SeedError> {
  running(kontur, State::ModuleOutage)
}

/// A member with a task in every claim state (`02h` `xUwYS`): the runs this computer has of them, which the QA app
/// joins with the server's claims, the claims it took here and the ones the server fenced. KON-11's fenced run was
/// interrupted, as a run taken over while its runner was gone is: it stays stopped, nothing continues it.
pub fn claims(kontur: &mut Kontur) -> Result<(), SeedError> {
  let runs = [
    (11, "Export invoices as PDF", Shape::Interrupted("implement"), 50),
    (12, "Add retry to webhook sender", Shape::Running("implement"), 40),
    (14, "Cache GitHub API responses", Shape::Succeeded, 90),
    (21, "Typed errors per module", Shape::Question("implement"), 25),
    (23, "Explain an expired GitHub token", Shape::Cancelled("implement"), 60),
    (24, "Import contacts from CSV", Shape::Interrupted("implement"), 70),
  ];
  with_runs(kontur, &runs)?;
  write_marker(kontur, State::Claims)
}

/// A member on the modules (`04d` `PqVUn`).
pub fn module_work(kontur: &mut Kontur) -> Result<(), SeedError> {
  mark(kontur, State::ModuleWork)
}

/// An Admin on the modules and Sharing (`04` `MvniR`, `07` `MnVch`).
pub fn admin_work(kontur: &mut Kontur) -> Result<(), SeedError> {
  mark(kontur, State::AdminWork)
}

/// As [`member`], the server refusing this version of Orchester (`07d` `BDJKM`).
pub fn version_refused(kontur: &mut Kontur) -> Result<(), SeedError> {
  mark(kontur, State::VersionRefused)
}

/// An Admin editing KON-12, whose first save meets olha's change of its priority first (`05` `LmxUY`).
pub fn edit_conflict(kontur: &mut Kontur) -> Result<(), SeedError> {
  mark(kontur, State::EditConflict)
}

/// As [`member`], with the team's servers, profiles and pipelines of `picker-readiness-team`; the stand-in server's
/// tasks are on Team delivery and Team quick fix. KON-5, in API rate limits, runs Team delivery, which needs the
/// program path of `pencil-design`, `github`'s secret and a connection: its Run is refused `not_ready_here` and its
/// Summary lists them (`Task · Run · not ready here`, R1 `Ku0qu`).
pub fn not_ready_here(kontur: &mut Kontur) -> Result<(), SeedError> {
  super::picker_readiness::team_work(kontur)?;
  // KON-5's module branch, which a team start finds in the checkout before it asks whether the pipeline is ready.
  git::branch(&kontur.repo, "module/api-rate-limits", git::MAIN)?;
  write_marker(kontur, State::NotReadyHere)
}

/// KON-12's run interrupted at Implement, on the live catalogue's Feature delivery with a brief that keeps
/// `orchester-fake-codex` working: Continue run takes it on, Running at 03 Implement with Pause run (`01c` `Tb55G`).
pub fn team_running(kontur: &mut Kontur) -> Result<(), SeedError> {
  let catalogue = kontur.live_catalogue()?;
  let pipeline = &catalogue.feature_delivery;
  let (title, brief) = (
    "Add retry to webhook sender",
    "Retry failed webhook sends with backoff. Keep working.",
  );
  numbered(kontur, 12)?;
  let mut task = kontur.task((title, brief, &[title]), Some(pipeline), kontur.ago(90))?;
  shapes::apply(
    kontur,
    &mut task,
    Some(pipeline),
    Shape::Interrupted("implement"),
    kontur.ago(40),
  )?;
  write_marker(kontur, State::TeamRunning)
}

/// KON-12 running Implement and KON-21 running Plan on this computer, and the marker naming `state`; in
/// `team-outage-running` their work reports wait to be sent.
fn running(kontur: &mut Kontur, state: State) -> Result<(), SeedError> {
  let catalogue = kontur.catalogue()?;
  let pipeline = &catalogue.feature_delivery;
  for (number, title, step, minutes, run, reports) in RUNNING {
    // As `numbered`, and the run counter too, as in a project whose earlier runs ended.
    let database = Connection::open(kontur.database())?;
    database.execute("UPDATE counters SET next = ?1 WHERE name = 'task'", [number])?;
    database.execute("UPDATE counters SET next = ?1 WHERE name = 'run'", [run])?;
    let (created, started) = (kontur.ago(minutes + 30), kontur.ago(minutes));
    let mut task = kontur.task((title, title, &[title]), Some(pipeline), created)?;
    shapes::apply(kontur, &mut task, Some(pipeline), Shape::Running(step), started)?;
    if matches!(state, State::OutageRunning) {
      waiting_reports(&database, &task, (started, reports))?;
    }
  }
  write_marker(kontur, state)
}

/// Kontur's tasks numbered as `runs` say, each with its title and its run in its shape, started minutes ago.
fn with_runs(kontur: &mut Kontur, runs: &[(u32, &str, Shape, i64)]) -> Result<(), SeedError> {
  let catalogue = kontur.catalogue()?;
  let pipeline = &catalogue.feature_delivery;
  for &(number, title, shape, minutes) in runs {
    numbered(kontur, number)?;
    let (created, started) = (kontur.ago(minutes + 30), kontur.ago(minutes));
    let mut task = kontur.task((title, title, &[title]), Some(pipeline), created)?;
    shapes::apply(kontur, &mut task, Some(pipeline), shape, started)?;
  }
  Ok(())
}

/// The next task made is numbered `number`. The store numbers tasks in order and has no API to number one: the seed
/// moves its counter on, as in a project whose earlier tasks are the server's.
fn numbered(kontur: &Kontur, number: u32) -> Result<(), SeedError> {
  Connection::open(kontur.database())?.execute("UPDATE counters SET next = ?1 WHERE name = 'task'", [number])?;
  Ok(())
}

/// `count` work reports of `task`'s run, started `at`, waiting in the outbox to be sent: the run recorded as started
/// through the team server, as the workspace's team work records it, with the fixture's empty JSON for what the start
/// and the reports send, which nothing sends while the server does not answer.
fn waiting_reports(database: &Connection, task: &Task, (at, count): (Timestamp, u32)) -> Result<(), SeedError> {
  let task = task.id().to_string();
  let run: String = database.query_row(
    "SELECT id FROM runs WHERE task_id = ?1 ORDER BY number DESC LIMIT 1",
    [&task],
    |row| row.get(0),
  )?;
  let (operation, work, at) = (
    OperationId::generate().to_string(),
    WorkId::generate().to_string(),
    at.unix_millis(),
  );
  database.execute(
    "INSERT INTO team_starts (operation_id, task_id, request, stage, work_id, bundle_sha256, run_id, created_at, \
     updated_at) VALUES (?1, ?2, '{}', 'started', ?3, ?4, ?5, ?6, ?6)",
    params![operation, task, work, "0".repeat(64), run, at],
  )?;
  database.execute(
    "INSERT INTO team_runs (run_id, operation_id, team_task_id, claim_id, work_id, next_sequence) \
     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
    params![run, operation, task, ClaimId::generate().to_string(), work, count + 1],
  )?;
  for sequence in 1..=count {
    database.execute(
      "INSERT INTO work_report_outbox (event_id, run_id, sequence, operation_id, outcome, request, state, created_at) \
       VALUES (?1, ?2, ?3, ?4, '{}', '{}', 'pending', ?5)",
      params![
        WorkEventId::generate().to_string(),
        run,
        sequence,
        OperationId::generate().to_string(),
        at
      ],
    )?;
  }
  Ok(())
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
