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
  /// The tasks the server could not be asked about, or did not answer.
  unreadable: HashSet<TaskId>,
}

/// The reading of each project, by its folder, with the tasks it was made for and when.
static READINGS: LazyLock<Mutex<HashMap<PathBuf, Kept>>> = LazyLock::new(Default::default);

struct Kept {
  reading: TeamReading,
  wanted: Vec<TaskId>,
  at: Instant,
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

  /// Whether some watched team task could not be read, so a module's tasks are not all known.
  pub(super) fn has_unreadable(&self) -> bool {
    !self.unreadable.is_empty()
  }

  /// Whether the subscription to `target` waits because the team server could not be asked about a team task it
  /// depends on: the task itself, or, for a module, any team task that may be in it.
  pub fn cannot_read(&self, target: WatchTarget) -> bool {
    match target {
      WatchTarget::Task(id) => self.unreadable.contains(&id),
      WatchTarget::Module(_) => self.has_unreadable(),
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

/// Reads the project at `root`'s watched team tasks from their team server unless a reading for the same tasks is
/// younger than `max_age`, and keeps it for [`TeamReading::of`]. Does nothing, and asks the server nothing, while no
/// subscription of the project needs a team task's status. Blocks on SQLite and, for a reading, on the server.
pub async fn refresh(servers: &Servers, root: &Path, max_age: Duration) -> Result<Refreshed, Message> {
  let folder = root.to_owned();
  let wanted = background::run(move || wanted_in(&folder))
    .await?
    .map_err(|error| messages::store_error(&error))?;
  let Some((link, tasks)) = wanted else {
    let dropped = READINGS.lock().remove(root);
    return Ok(Refreshed {
      changed: dropped.is_some_and(|kept| kept.reading != TeamReading::default()),
      polling: false,
    });
  };
  let young = READINGS
    .lock()
    .get(root)
    .is_some_and(|kept| kept.wanted == tasks && kept.at.elapsed() < max_age);
  if young {
    return Ok(Refreshed {
      changed: false,
      polling: true,
    });
  }
  let reading = read::tasks(servers, &link, &tasks).await;
  let before = READINGS.lock().insert(
    root.to_owned(),
    Kept {
      reading: reading.clone(),
      wanted: tasks,
      at: Instant::now(),
    },
  );
  Ok(Refreshed {
    changed: before.is_none_or(|before| before.reading != reading),
    polling: true,
  })
}

/// The server project the project at `root` is linked to and the team tasks whose status its subscriptions need, in
/// the order the project recorded them: those a task subscription names, or all of the project's when a module is
/// watched (the server says which module each is in). `None` when no subscription needs any.
fn wanted_in(root: &Path) -> Result<Option<(SharingLink, Vec<TaskId>)>, StoreError> {
  let db = ProjectDb::open(root)?;
  let Some(link) = db.sharing()? else {
    return Ok(None);
  };
  let subscriptions = db.active_subscriptions()?;
  let targets: Vec<WatchTarget> = subscriptions.as_slice().iter().map(Subscription::target).collect();
  let modules = targets.iter().any(|target| matches!(target, WatchTarget::Module(_)));
  let tasks: Vec<TaskId> = db
    .team_task_ids()?
    .into_iter()
    .filter(|id| modules || targets.contains(&WatchTarget::Task(*id)))
    .collect();
  Ok((!tasks.is_empty()).then_some((link, tasks)))
}
