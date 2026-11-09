//! What the team server says of the team tasks the project's subscriptions watch (`docs/design/orchestrator-watch.md`,
//! *Team tasks*). A subscription to a task or a module ends on workflow status (Done or Canceled), which the team
//! server owns for a team task: this computer keeps the task's identity and its runs, and no copy of its status it may
//! trust. So the status, and the module the server puts the task in, are read from the server in the background
//! ([`refresh`]) for as long as such a subscription exists, and the ending rule ([`super::watch_ends`]) reads that
//! reading, never a stored row of a team task.
//!
//! Nothing pushes into the workspace (the team server's invalidations reach the app's own team reads only), so the
//! app asks again every [`POLL`] while [`Refreshed::polling`] says a subscription needs it. A reading that cannot be
//! made (signed out, the server unreachable, a refusal) is not an answer: the subscription goes on, and its listing
//! says why ([`TeamReading::cannot_read`]) until a later reading sees the task.
//!
//! The cost of a reading is bounded by what is watched, never by the project's size: one `GET tasks/{id}` for each
//! team task a *task* subscription names, and for each watched *module* one request of the task list, filtered to that
//! module, per `MAX_PAGE_LIMIT` (200) tasks the module holds. A pass stops at the first sign that the server cannot be
//! asked (signed out, unreachable, a membership that ended); a task it refuses or cannot decode leaves only that task,
//! or that module, unread.

mod read;

use std::{
  collections::{HashMap, HashSet},
  path::{Path, PathBuf},
  sync::LazyLock,
  time::{Duration, Instant},
};

use orchester_core::{
  id::{ModuleId, TaskId},
  orchestrator::{Subscription, WatchTarget},
  project::TaskKey,
  task::organization::WorkflowStatus,
};
use orchester_store::{ProjectDb, SharingLink, StoreError};
use parking_lot::Mutex;

use crate::{
  background,
  messages::{self, Message},
  sharing::Servers,
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
}

/// The last reading of a project's watched team tasks.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TeamReading {
  tasks: HashMap<TaskId, TeamTask>,
  /// The watched modules whose recorded team tasks the server listed in full.
  listed: HashSet<ModuleId>,
  /// The project's team tasks when the reading was made: one recorded since is not in it, and the reading says nothing
  /// of which module it is in.
  covered: HashSet<TaskId>,
  /// The team tasks a task subscription names that the server could not be asked about, or did not answer.
  unreadable: HashSet<TaskId>,
  /// The watched modules whose tasks the server could not be asked to list, or did not answer.
  unreadable_modules: HashSet<ModuleId>,
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
}

/// The reading of each project, by its folder, with what it was made for and when it began.
static READINGS: LazyLock<Mutex<HashMap<PathBuf, Kept>>> = LazyLock::new(Default::default);

struct Kept {
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

  pub(super) fn tasks(&self) -> impl Iterator<Item = &TeamTask> {
    self.tasks.values()
  }

  /// Whether the team tasks in `module` are all known, given the project's team tasks `team` now: the project has
  /// none, or the server listed the module in a reading made when every one of them was recorded.
  pub(super) fn knows_module(&self, module: ModuleId, team: &HashSet<TaskId>) -> bool {
    team.is_empty() || (self.listed.contains(&module) && team.is_subset(&self.covered))
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
  let Some((link, wants)) = wanted else {
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
  let reading = read::pass(servers, &link, &wants).await;
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
/// is in). `None` when no subscription needs any, or the project has no team task recorded.
fn wanted_in(root: &Path) -> Result<Option<(SharingLink, Wants)>, StoreError> {
  let db = ProjectDb::open(root)?;
  let Some(link) = db.sharing()? else {
    return Ok(None);
  };
  let team = db.team_task_ids()?;
  if team.is_empty() {
    return Ok(None);
  }
  let subscriptions = db.active_subscriptions()?;
  let mut wants = Wants::default();
  for target in subscriptions.as_slice().iter().map(Subscription::target) {
    match target {
      WatchTarget::Task(id) if team.contains(&id) && !wants.tasks.contains(&id) => wants.tasks.push(id),
      WatchTarget::Module(id) if !wants.modules.contains(&id) => wants.modules.push(id),
      _ => {}
    }
  }
  wants.team = team;
  Ok((!wants.tasks.is_empty() || !wants.modules.is_empty()).then_some((link, wants)))
}
