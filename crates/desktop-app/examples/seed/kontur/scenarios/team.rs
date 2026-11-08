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
  connection::{AgentProvider, SignIn},
  id::{ClaimId, OperationId, ProjectId, WorkEventId, WorkId},
  project::{Project, ProjectKey},
  resource::ResourceKind,
  store::{ResourceStore, TaskStore},
  task::Task,
  time::Timestamp,
};
use orchester_store::{AppDb, ArchiveStop, Archived, ProjectDb};
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

/// As [`not_ready_here`], with a Claude Code connection, so Team delivery lacks only the program path of
/// `pencil-design` and the value of `GITHUB_TOKEN`, and KON-5 having a run on this computer that was cancelled at
/// Implement, run 8 (the run counter as in a project whose earlier runs ended), while mara's claim on it is stale
/// (`team-not-ready-ended`): a Take over starts a new run, which is refused as not ready here, and the header's line
/// says when run 8 ended beside when Take over was pressed (`Status line` `g9ufW`, ORC-840b).
pub fn not_ready_after_run(kontur: &mut Kontur) -> Result<(), SeedError> {
  super::picker_readiness::team_work(kontur)?;
  claude_connection(kontur)?;
  git::branch(&kontur.repo, "module/api-rate-limits", git::MAIN)?;
  Connection::open(kontur.database())?.execute("UPDATE counters SET next = 8 WHERE name = 'run'", [])?;
  with_runs(
    kontur,
    &[(5, "Rotate API keys from settings", Shape::Cancelled("implement"), 90)],
  )?;
  write_marker(kontur, State::NotReadyAfterRun)
}

/// As [`not_ready_here`], with every task of the stand-in server on Team quick fix, which is ready here, and the server
/// a release ahead of this computer (`team-start-failed`): KON-5 (API rate limits) is pressed Run; the server builds
/// the start in a bundle schema this computer does not read, so the start fails for good, the claim is released and the
/// task page says why (ORC-885).
pub fn start_failed(kontur: &mut Kontur) -> Result<(), SeedError> {
  start_ready_here(kontur, State::StartFailed)
}

/// As [`start_failed`], with the server not answering the Run's start for a while (`team-start-recovered`): the start
/// waits, and recovery carries it on when the server answers, so the page that said the run starts once they answer
/// says why it did not.
pub fn start_recovered(kontur: &mut Kontur) -> Result<(), SeedError> {
  start_ready_here(kontur, State::StartRecovered)
}

/// As [`start_failed`], with KON-5's start already recorded when the QA app joins the team, as the server's not
/// answering it leaves it (`team-start-waiting`): the bundle is never answered, recovery goes on asking, and the task
/// page says the run is starting, in an information notice with nothing to press (ORC-885c).
pub fn start_waiting(kontur: &mut Kontur) -> Result<(), SeedError> {
  start_ready_here(kontur, State::StartWaiting)
}

/// As [`start_failed`], with KON-5's start recorded at the runner when the QA app joins the team
/// (`team-start-runner-refused`): the runner, which keeps no inputs for it, refuses it when recovery asks, and the task
/// page says what the runner said without a press of Run (ORC-885c).
pub fn start_runner_refused(kontur: &mut Kontur) -> Result<(), SeedError> {
  start_ready_here(kontur, State::StartRunnerRefused)
}

/// As [`start_failed`], with KON-5's start recorded as an earlier window left it, failed for good and not dismissed
/// (`team-start-past-failure`): the task page tells it once, with Try again and Dismiss (ORC-885c).
pub fn start_past_failure(kontur: &mut Kontur) -> Result<(), SeedError> {
  start_ready_here(kontur, State::StartPastFailure)
}

fn start_ready_here(kontur: &mut Kontur, state: State) -> Result<(), SeedError> {
  super::picker_readiness::team_work(kontur)?;
  // Team quick fix runs Reviewer, which needs a Claude Code connection here; with one it is ready.
  claude_connection(kontur)?;
  // KON-5's module branch, which a team start finds in the checkout before it asks whether the pipeline is ready.
  git::branch(&kontur.repo, "module/api-rate-limits", git::MAIN)?;
  write_marker(kontur, state)
}

/// The Claude Code connection `Claude · Kontur`, which the team's profiles run on without asking.
fn claude_connection(kontur: &mut Kontur) -> Result<(), SeedError> {
  let bin = kontur.repo.join(".orchester").join("qa-bin");
  let program = bin.join("claude");
  fs::create_dir_all(&bin).map_err(io(&bin))?;
  fs::write(&program, "").map_err(io(&program))?;
  let at = kontur.ago(4);
  super::connections::cli(
    kontur,
    ("Claude · Kontur", AgentProvider::ClaudeCodeCli, program),
    ("2.1.89", SignIn::SignedIn, &["claude-sonnet-5-5"]),
    at,
  )
  .map(|_| ())
}

/// The team's resources of `team-hub` for a copy that joins the stand-in team server as an Admin, which holds the
/// team's MCP servers (`team-edit`, `team-edit-demoted`, `team-edit-offline`, `team-edit-rejects`, ORC-836): `Agents ›
/// MCP servers`, *Edit* a server (`Agents · MCP server · admin · draft` A13 `WMtxm`).
pub fn edit(kontur: &mut Kontur) -> Result<(), SeedError> {
  edit_in(kontur, State::TeamEdit)
}

/// As [`edit`], the server no longer taking the person's writes: *Publish for the team* is refused as no longer an
/// admin, and the page turns read-only with the draft kept (A14 `gTLx6`).
pub fn edit_demoted(kontur: &mut Kontur) -> Result<(), SeedError> {
  edit_in(kontur, State::TeamEditDemoted)
}

/// As [`edit`], the server not answering: *Publish for the team* waits in the outbox, with *Cancel sending* (A16
/// `x3ywL`).
pub fn edit_offline(kontur: &mut Kontur) -> Result<(), SeedError> {
  edit_in(kontur, State::TeamEditOffline)
}

/// As [`edit`], the server rejecting the content of what is sent: the refusal says why and the draft is kept (A17
/// `rJbPm`).
pub fn edit_rejects(kontur: &mut Kontur) -> Result<(), SeedError> {
  edit_in(kontur, State::TeamEditRejects)
}

/// As [`edit`], after *Publish for the team* made `pencil-design`'s next version and the server numbered it: v2 is the
/// team's and the draft became it (`team-edit-published`, the result of A13 `WMtxm`). The QA app makes it without a
/// press.
pub fn edit_published(kontur: &mut Kontur) -> Result<(), SeedError> {
  edit_in(kontur, State::TeamEditPublished)
}

/// As [`edit`], after a *Publish for the team* refused as no longer an admin, the person a member now: the page is
/// read-only with the draft kept (`team-edit-refused-admin`, A14 `gTLx6`). The program path of `pencil-design` is set,
/// so its row says only *Your draft on v1 · publish refused*.
pub fn edit_refused_admin(kontur: &mut Kontur) -> Result<(), SeedError> {
  edit_in(kontur, State::TeamEditRefusedAdmin)
}

/// As [`edit`], after a *Publish for the team* made while the server could not be reached: v2 waits in the outbox, with
/// *Cancel sending* (`team-edit-waiting`, A16 `x3ywL`). The program path is set, so the row says only *Waiting to send
/// · v2*.
pub fn edit_waiting(kontur: &mut Kontur) -> Result<(), SeedError> {
  edit_in(kontur, State::TeamEditWaiting)
}

/// As [`edit`], a version that waited and that the server rejected once it answered: the outbox holds it refused, with
/// the reason, the form of the refused version and *Publish for the team* (`team-edit-rejected`, A17 `rJbPm`). The
/// program path is set, so the row says only *Your draft on v1 · publish refused*.
pub fn edit_rejected(kontur: &mut Kontur) -> Result<(), SeedError> {
  edit_in(kontur, State::TeamEditRejected)
}

/// As [`edit`], `pencil-design` needing the variable `PENCIL_TOKEN`: the edit form has *Variables it needs* (A13
/// `WMtxm`).
pub fn edit_variables(kontur: &mut Kontur) -> Result<(), SeedError> {
  super::team_hub::resources_mcp(
    kontur,
    &super::team_hub::Mcp {
      pencil_variables: &["PENCIL_TOKEN"],
      figma_program: false,
    },
  )?;
  write_marker(kontur, State::TeamEditVariables)
}

/// As [`edit`], a new MCP server `figma-local` that waited and that the server refused as the name being taken once it
/// answered: taken back whole, and the hub says what and why (`team-create-name-taken`, A15 `dLqHC`).
pub fn create_name_taken(kontur: &mut Kontur) -> Result<(), SeedError> {
  edit_in(kontur, State::TeamCreateNameTaken)
}

/// As [`edit`], with a draft of `pencil-design` kept on v1 and olha's v2 published meanwhile and read here: one quiet
/// line says so, with *View changes* (`team-edit-newer`, A18 `bk4i9`). The QA app makes it without a press.
pub fn edit_newer(kontur: &mut Kontur) -> Result<(), SeedError> {
  edit_in(kontur, State::TeamEditNewer)
}

/// As [`edit_newer`], after a *Publish for the team* that the server answered with the conflict: the page compares v1,
/// olha's v2 and the draft field by field, the arguments both changed (`team-edit-conflicting`, A19 `DV8OI`; *Discard
/// my draft* opens A20 `CAPhQ`). The QA app makes it without a press.
pub fn edit_conflicting(kontur: &mut Kontur) -> Result<(), SeedError> {
  edit_in(kontur, State::TeamEditConflicting)
}

/// As [`edit_conflicting`], with no field both changed, so *Rebase my draft* needs no pick
/// (`team-edit-conflict-clean`).
pub fn edit_conflict_clean(kontur: &mut Kontur) -> Result<(), SeedError> {
  edit_in(kontur, State::TeamEditConflictClean)
}

/// As [`edit`], olha archived `pencil-design` while the draft was kept and the *Publish for the team* that followed was
/// refused: the draft stays, *Restore* is the way back (`team-edit-archived`, A21 `yqYj9`). The QA app makes it without
/// a press.
pub fn edit_archived(kontur: &mut Kontur) -> Result<(), SeedError> {
  edit_in(kontur, State::TeamEditArchived)
}

fn edit_in(kontur: &mut Kontur, state: State) -> Result<(), SeedError> {
  super::team_hub::resources(kontur)?;
  write_marker(kontur, state)
}

/// An Admin on the hub's team MCP servers, which the stand-in team server holds with Builder, that names
/// `pencil-design` (`team-archive`, A23 `Pnpbe`, ORC-838): *Agents › MCP servers › pencil-design*, the … menu's
/// *Archive*, which opens the confirmation. Opening it is a press an MCP client may make (`team-hub-more`, then
/// `team-hub-archive`); confirming it is a person's.
pub fn archive(kontur: &mut Kontur) -> Result<(), SeedError> {
  super::team_hub::resources(kontur)?;
  write_marker(kontur, State::TeamArchive)
}

/// What the confirmation of [`archive`] leaves behind (`team-archived`, A22 `pUu78`): KON-12's run 8, cancelled at
/// Implement, recorded as stopped when dana-k archived `pencil-design`, which the QA app then takes out of this
/// computer's lists as the server's archive does. The task's Summary says which resource and who archived it, and
/// Builder, which still names it, cannot run.
pub fn archived(kontur: &mut Kontur) -> Result<(), SeedError> {
  super::picker_readiness::team_work(kontur)?;
  Connection::open(kontur.database())?.execute("UPDATE counters SET next = 8 WHERE name = 'run'", [])?;
  with_runs(
    kontur,
    &[(12, "Add retry to webhook sender", Shape::Cancelled("implement"), 60)],
  )?;
  let (run, task): (String, String) = Connection::open(kontur.database())?.query_row(
    "SELECT id, task_id FROM runs ORDER BY number DESC LIMIT 1",
    [],
    |row| Ok((row.get(0)?, row.get(1)?)),
  )?;
  on_team_delivery(
    kontur,
    task
      .parse()
      .map_err(|_| SeedError::Invalid("the seed's task is an id".to_owned()))?,
  )?;
  let pencil = kontur
    .db
    .resources(ResourceKind::McpServer)?
    .into_iter()
    .find(|resource| resource.name().as_str() == "pencil-design")
    .ok_or_else(|| SeedError::Invalid("the seed has the team MCP server pencil-design".to_owned()))?;
  let invalid = |what: &str| SeedError::Invalid(format!("the seed's {what} is an id"));
  kontur.db.record_archive_stop(&ArchiveStop {
    run: run.parse().map_err(|_| invalid("run"))?,
    task: task.parse().map_err(|_| invalid("task"))?,
    resource: pencil.name().clone(),
    stopped_at: kontur.ago(60),
    archived: Some(Archived {
      id: pencil.id(),
      kind: ResourceKind::McpServer,
      by: Some("dana-k".to_owned()),
    }),
  })?;
  write_marker(kontur, State::TeamArchived)
}

/// KON-12 moved from the catalogue's pipeline, which a team project does not offer, to the team's Team delivery
/// (Builder, then Reviewer), so a Run again reads the profiles of the team and says what keeps them from running; its
/// run 8 is still the one the catalogue's Feature delivery made.
fn on_team_delivery(kontur: &mut Kontur, task: orchester_core::id::TaskId) -> Result<(), SeedError> {
  let delivery = kontur
    .db
    .resources(ResourceKind::Pipeline)?
    .into_iter()
    .find(|resource| resource.name().as_str() == "Team delivery")
    .ok_or_else(|| SeedError::Invalid("the seed has the team pipeline Team delivery".to_owned()))?;
  let mut found = kontur
    .db
    .task(task)?
    .ok_or_else(|| SeedError::Invalid("the seed has KON-12".to_owned()))?;
  let mut details = found.details().clone();
  details.pipeline = Some(delivery.id());
  found.edit(details, true).map_err(|_| SeedError::TaskChanged)?;
  kontur.db.save_task(&found)?.saved().ok_or(SeedError::TaskChanged)?;
  Ok(())
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
