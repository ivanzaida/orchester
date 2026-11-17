//! What the team server says of the team tasks the project's subscriptions watch (`docs/design/orchestrator-watch.md`,
//! *Team tasks*). A subscription to a task or a module ends on workflow status (Done or Canceled), which the team
//! server owns for a team task: this computer keeps the task's identity and its runs, and no copy of its status it may
//! trust. So the status, and the module the server puts the task in, are read from the server in the background
//! ([`refresh`]) for as long as such a subscription exists, and the ending rule ([`super::watch_ends`]) reads that
//! reading, never a stored row of a team task. Who leads a task reads the same reading for the module the task is in
//! now ([`module_now`]), so a task the server moved is led by where it is, not by where its last run started.
//!
//! Nothing pushes into the workspace (the team server's invalidations reach the app's own team reads only), so the
//! app asks again every [`POLL`] while [`Refreshed::polling`] says a subscription needs it. A reading that cannot be
//! made (signed out, the server unreachable, a refusal) is not an answer: the subscription goes on, and its listing
//! says why ([`TeamReading::cannot_read`]) until a later reading sees the task.
//!
//! A watched module is read whole: every task the server puts in it, whether or not it ever ran on this computer, so
//! a conversation that leads the module knows the tasks it has still to start
//! (`docs/plan/orchestrator-autonomy/OA-28-…`). So are the tasks without a module and the whole project while a
//! subscription watches them: their Blocked marks and statuses are the server's, as a watched module's are. The
//! reading keeps which team pipelines this session read too, so where a led target stands says whether a team task's
//! pipeline is ready here, as it says it of a local task's.
//!
//! The cost of a reading is bounded by what is watched: one `GET tasks/{id}` for each team task a *task* subscription
//! names, for each watched *module* one request of the task list, filtered to that module, per `MAX_PAGE_LIMIT` (200)
//! tasks the module holds, and while the tasks without a module (or the whole project) are watched, one request per
//! 200 of those tasks. A pass stops at the first sign that the server cannot be asked (signed out, unreachable, a
//! membership that ended); a task it refuses or cannot decode leaves only that task, or that list, unread.

mod read;

use std::{
  collections::{HashMap, HashSet},
  path::{Path, PathBuf},
  sync::LazyLock,
  time::{Duration, Instant},
};

use orchester_core::{
  id::{ModuleId, ProjectId, TaskId},
  orchestrator::{Subscription, WatchTarget, standing::Unready},
  project::TaskKey,
  task::{Task, organization::WorkflowStatus},
};
use orchester_store::{ProjectDb, SharingLink, StoreError};
use parking_lot::Mutex;

use crate::{
  background,
  messages::{self, Message},
  sharing::Servers,
  team_reads::listed::TeamPipelines,
};

/// How often the app asks the team server again while a subscription needs its answer.
pub const POLL: Duration = Duration::from_secs(60);

/// A team task as the team server read it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TeamTask {
  pub key: TaskKey,
  pub title: String,
  pub status: WorkflowStatus,
  /// The module the server puts the task in.
  pub module: Option<ModuleId>,
  /// Why the server holds it Blocked; `None` while it does not.
  pub blocked: Option<String>,
  /// The team pipeline the task names, as the server keeps it; `None` while it names none.
  pub pipeline: Option<String>,
  /// The team tasks that block it (OA-35), by their server ids.
  pub blocking: Vec<TaskId>,
}

/// The last reading of a project's watched team tasks.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TeamReading {
  tasks: HashMap<TaskId, TeamTask>,
  /// The watched modules whose tasks the server listed in full.
  listed: HashSet<ModuleId>,
  /// The project's team tasks when the reading was made: one recorded since is not in it, and the reading says nothing
  /// of which module it is in.
  covered: HashSet<TaskId>,
  /// The team tasks a task subscription names that the server could not be asked about, or did not answer.
  unreadable: HashSet<TaskId>,
  /// The watched modules whose tasks the server could not be asked to list, or did not answer.
  unreadable_modules: HashSet<ModuleId>,
  /// The team's pipelines as this session read its configuration when the reading was made: which pipeline a team
  /// task names is ready here, as a local task's published pipeline is.
  pipelines: TeamPipelines,
}

/// What a reading is made for: the project's recorded team tasks, and of them and the project's modules what the
/// subscriptions watch.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Wants {
  /// Every team task the project recorded, in the order it recorded them.
  team: Vec<TaskId>,
  /// The team tasks a task subscription names, each read by itself.
  tasks: Vec<TaskId>,
  /// The modules a module subscription names, each read as the server's list of its tasks.
  modules: Vec<ModuleId>,
  /// What a subscription to the tasks without a module, or to the whole project, watches: read as the server's list
  /// of those tasks. `None` while no such subscription exists.
  lists: Option<Listed>,
}

/// The tasks a subscription that names no module or task watches, read as the server's list of them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Listed {
  /// The tasks in no module.
  WithoutModule,
  /// Every task of the project, which holds the tasks in no module too.
  Project,
}

/// The reading of each project, by its folder, with what it was made for and when it began.
static READINGS: LazyLock<Mutex<HashMap<PathBuf, Kept>>> = LazyLock::new(Default::default);

struct Kept {
  /// The project the reading is of, by which a reader that holds its database finds it.
  project: ProjectId,
  reading: TeamReading,
  wanted: Wants,
  /// When the reading began: one that began earlier never replaces one that began later.
  began: Instant,
}

impl TeamReading {
  /// The last reading of the project at `root`; empty before the first, and while no subscription needs one.
  pub fn of(root: &Path) -> Self {
    READINGS
      .lock()
      .get(root)
      .map(|kept| kept.reading.clone())
      .unwrap_or_default()
  }

  pub(super) fn task(&self, id: TaskId) -> Option<&TeamTask> {
    self.tasks.get(&id)
  }

  /// The keys of the tasks blocking team task `id` that this reading saw not Done, as a local task's unfinished
  /// blocking tasks are read (OA-35: a team task is Done once its work is merged). A blocking task the reading did
  /// not see is not told here; `run_task` reads every one of them on the server before a start.
  pub(super) fn unfinished_blockers(&self, id: TaskId) -> Vec<String> {
    let Some(task) = self.tasks.get(&id) else {
      return Vec::new();
    };
    task
      .blocking
      .iter()
      .filter_map(|blocker| self.tasks.get(blocker))
      .filter(|blocker| blocker.status != WorkflowStatus::Done)
      .map(|blocker| blocker.key.to_string())
      .collect()
  }

  pub(super) fn tasks(&self) -> impl Iterator<Item = &TeamTask> {
    self.tasks.values()
  }

  /// Every task the reading saw, by its id, in the order of their keys: the same order at every reading.
  pub(super) fn seen(&self) -> Vec<(TaskId, &TeamTask)> {
    let mut seen: Vec<(TaskId, &TeamTask)> = self.tasks.iter().map(|(id, task)| (*id, task)).collect();
    seen.sort_by_key(|(_, task)| task.key.number());
    seen
  }

  /// The workflow status the team server last gave team task `id`; `None` while no reading saw it.
  pub(super) fn status_of(&self, id: TaskId) -> Option<WorkflowStatus> {
    self.tasks.get(&id).map(|seen| seen.status)
  }

  /// Whether a team task that names `pipeline` (as the server keeps it) can start here as far as its pipeline goes: it
  /// names one, and that one is among the team's pipelines this session read, with a published version, as a local
  /// task's pipeline must be published. While the configuration is not read, a pipeline it names is taken as ready:
  /// the start checks it again.
  pub(super) fn pipeline_unready(&self, pipeline: Option<&str>) -> Option<Unready> {
    match pipeline {
      None => Some(Unready::NoPipeline),
      Some(named) if self.pipelines.id(named).is_none() => Some(Unready::PipelineNotReady),
      Some(_) => None,
    }
  }

  /// The module `task` is in now, for who leads it: where the team server put it, when this reading saw the task; the
  /// module its records name otherwise (a local task's own; a team task's, the one its newest run started in). A team
  /// task the server moved out of every watched module is in no reading, and stays where its records put it until it
  /// runs again.
  pub(super) fn module_of(&self, task: &Task) -> Option<ModuleId> {
    self
      .tasks
      .get(&task.id())
      .map_or_else(|| task.module(), |seen| seen.module)
  }

  /// Whether the team tasks in `module` are all known, given the project's team tasks `team` now: the project is not
  /// `linked` to a team server, or the server listed the module in a reading made when every one of them was recorded.
  pub(super) fn knows_module(&self, module: ModuleId, team: &HashSet<TaskId>, linked: bool) -> bool {
    !linked || (self.listed.contains(&module) && team.is_subset(&self.covered))
  }

  /// Whether the subscription to `target` waits because the team server could not be asked about what it depends on:
  /// the team task itself, or, for a module, the list of its tasks.
  pub fn cannot_read(&self, target: WatchTarget) -> bool {
    match target {
      WatchTarget::Task(id) => self.unreadable.contains(&id),
      WatchTarget::Module(id) => self.unreadable_modules.contains(&id),
      WatchTarget::WithoutModule | WatchTarget::Project => false,
    }
  }
}

/// The module `task` is in now, for who leads it, by the last reading of `db`'s project ([`TeamReading::module_of`]);
/// the module its records name while the project has no reading.
pub(super) fn module_now(db: &ProjectDb, task: &Task) -> Option<ModuleId> {
  reading_of(db).module_of(task)
}

/// The last reading of `db`'s project; empty before the first, and while no subscription needs one.
pub(super) fn reading_of(db: &ProjectDb) -> TeamReading {
  let project = db.project().id();
  READINGS
    .lock()
    .values()
    .find(|kept| kept.project == project)
    .map(|kept| kept.reading.clone())
    .unwrap_or_default()
}

/// What a [`refresh`] found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Refreshed {
  /// The reading differs from the one before: a status, a module or what cannot be read changed.
  pub changed: bool,
  /// Some subscription needs the team server's answer, so the app asks again after [`POLL`].
  pub polling: bool,
}

/// Reads the project at `root`'s watched team tasks from their team server unless a reading for the same wants is
/// younger than `max_age`, and keeps it for [`TeamReading::of`]. Does nothing, and asks the server nothing, while no
/// subscription of the project needs a team task's status. Blocks on SQLite and, for a reading, on the server.
pub async fn refresh(servers: &Servers, root: &Path, max_age: Duration) -> Result<Refreshed, Message> {
  let folder = root.to_owned();
  let wanted = background::run(move || wanted_in(&folder))
    .await?
    .map_err(|error| messages::store_error(&error))?;
  let Some((project, link, wants)) = wanted else {
    let dropped = READINGS.lock().remove(root);
    return Ok(Refreshed {
      changed: dropped.is_some_and(|kept| kept.reading != TeamReading::default()),
      polling: false,
    });
  };
  let young = READINGS
    .lock()
    .get(root)
    .is_some_and(|kept| kept.wanted == wants && kept.began.elapsed() < max_age);
  if young {
    return Ok(Refreshed {
      changed: false,
      polling: true,
    });
  }
  let began = Instant::now();
  let mut reading = read::pass(servers, &link, &wants).await;
  // A configuration not read yet leaves the team's pipelines unread: a pipeline a task names reads as ready until then.
  if let Ok(config) = servers.team_read(root).loaded() {
    reading.pipelines = TeamPipelines::of(&config);
  }
  let mut readings = READINGS.lock();
  if readings.get(root).is_some_and(|kept| kept.began > began) {
    // Another refresh began after this one and finished first: its reading is the newer.
    return Ok(Refreshed {
      changed: false,
      polling: true,
    });
  }
  let before = readings.insert(
    root.to_owned(),
    Kept {
      project,
      reading: reading.clone(),
      wanted: wants,
      began,
    },
  );
  Ok(Refreshed {
    changed: before.is_none_or(|before| before.reading != reading),
    polling: true,
  })
}

/// Drops the reading of the project at `root`, which is closed: nothing reads it again until it is opened.
pub fn forget(root: &Path) {
  READINGS.lock().remove(root);
}

/// The server project the project at `root` is linked to and what its subscriptions need of its team tasks: those a
/// task subscription names, and the modules a module subscription names (the server says which module each team task
/// is in), with the project's id. `None` when no subscription needs any. A watched module is read though no team task
/// ran here yet: the server may hold tasks in it.
fn wanted_in(root: &Path) -> Result<Option<(ProjectId, SharingLink, Wants)>, StoreError> {
  let db = ProjectDb::open(root)?;
  let Some(link) = db.sharing()? else {
    return Ok(None);
  };
  let team = db.team_task_ids()?;
  let subscriptions = db.active_subscriptions()?;
  let mut wants = Wants::default();
  for target in subscriptions.as_slice().iter().map(Subscription::target) {
    match target {
      WatchTarget::Task(id) if team.contains(&id) && !wants.tasks.contains(&id) => wants.tasks.push(id),
      WatchTarget::Module(id) if !wants.modules.contains(&id) => wants.modules.push(id),
      WatchTarget::Project => wants.lists = Some(Listed::Project),
      WatchTarget::WithoutModule if wants.lists.is_none() => wants.lists = Some(Listed::WithoutModule),
      WatchTarget::Task(_) | WatchTarget::Module(_) | WatchTarget::WithoutModule => {}
    }
  }
  wants.team = team;
  let needed = !wants.tasks.is_empty() || !wants.modules.is_empty() || wants.lists.is_some();
  Ok(needed.then_some((db.project().id(), link, wants)))
}
