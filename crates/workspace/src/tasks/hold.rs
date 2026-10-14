//! A person's hold on new starts (`docs/design/orchester-workspace.md`, "Pause scheduling"): in the whole project or
//! in one reviewed group, kept in the project's database, where the runner reads it before it starts a first run.
//! Only new starts wait: running runs go on, and so do resumes and runs again. The Tasks screen, Orchester's MCP
//! server and the Orchestrator chat hold and release the queue through [`hold_in`], with the same rules.

use std::path::Path;

use orchester_core::{
  id::{IdempotencyKey, TaskId},
  orchestrator::HeldWork,
  store::{RunStore, TaskStore},
  task::TaskStatus,
  task_group::{HoldActor, HoldScope},
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};

use crate::messages::{self, Message};

/// The holds on new starts as the Tasks screen shows them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HeldQueue {
  /// New starts are held in the whole project.
  pub project: bool,
  /// The groups whose new starts are held, each with its tasks' keys in their reviewed order.
  pub groups: Vec<HeldGroup>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HeldGroup {
  pub key: IdempotencyKey,
  pub tasks: Vec<String>,
}

/// The holds on new starts in `db`, each group with its tasks' keys.
pub fn held_in(db: &ProjectDb) -> Result<HeldQueue, StoreError> {
  let holds = db.queue_holds()?;
  let mut groups = Vec::with_capacity(holds.groups.len());
  for key in holds.groups {
    let tasks = match db.task_group(key)? {
      Some(group) => group
        .tasks()
        .iter()
        .filter_map(|task| db.task(*task).ok().flatten())
        .map(|task| task.key().to_string())
        .collect(),
      None => Vec::new(),
    };
    groups.push(HeldGroup { key, tasks });
  }
  Ok(HeldQueue {
    project: holds.project,
    groups,
  })
}

/// Where a hold on `task`'s new starts would apply: the reviewed group it belongs to; `None` when it is in none.
pub fn group_of(db: &ProjectDb, task: TaskId) -> Result<Option<IdempotencyKey>, StoreError> {
  Ok(db.task_group_for_task(task)?.map(|group| group.key()))
}

/// What a hold on new starts in `group` (the whole project when `None`) holds now: the tasks waiting to start, queued
/// without a run, in its order (the group's reviewed order, else the project's).
pub fn held_work(db: &ProjectDb, group: Option<IdempotencyKey>) -> Result<HeldWork, StoreError> {
  let tasks = match group {
    Some(key) => {
      let ids = db
        .task_group(key)?
        .map(|group| group.tasks().to_vec())
        .unwrap_or_default();
      let mut tasks = Vec::new();
      for id in ids {
        tasks.extend(db.task(id)?);
      }
      tasks
    }
    None => db.tasks()?,
  };
  let mut waiting = Vec::new();
  for task in tasks {
    if task.status() == TaskStatus::Queued && db.runs_of_task(task.id()).map_err(run_store)?.is_empty() {
      waiting.push(task.key().to_string());
    }
  }
  Ok(HeldWork { group, waiting })
}

/// A run store error as the store error it is; one about a run itself is a corrupt record here.
fn run_store(error: orchester_store::RunStoreError) -> StoreError {
  match error {
    orchester_store::RunStoreError::Store(error) => error,
    other => StoreError::Corrupt {
      what: "run",
      detail: other.to_string(),
    },
  }
}

/// The reviewed group of the task named `named`, by its key or id; `None` when it is in none or is not found.
pub fn group_of_named(db: &ProjectDb, named: &str) -> Result<Option<IdempotencyKey>, StoreError> {
  let lowered = named.to_lowercase();
  let task = db
    .tasks()?
    .into_iter()
    .find(|task| task.key().to_string().to_lowercase() == lowered || task.id().to_string() == named);
  match task {
    Some(task) => group_of(db, task.id()),
    None => Ok(None),
  }
}

/// Why a hold was not changed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HoldRefused {
  /// A person held new starts there: only a person releases them.
  PersonOnly,
  Store(Message),
}

impl HoldRefused {
  /// What the person or the client reads.
  pub fn message(self) -> Message {
    match self {
      Self::PersonOnly => Message::new("tasks.queue.personHeld"),
      Self::Store(message) => message,
    }
  }
}

/// Holds new starts in `scope` of the project at `root` as `actor` (`held`), or lets them go on; whether that changed
/// anything. Automation does not release a hold a person set. Blocks on SQLite.
pub fn hold_in(root: &Path, (scope, actor): (HoldScope, HoldActor), held: bool) -> Result<bool, HoldRefused> {
  let store = |error: StoreError| HoldRefused::Store(messages::store_error(&error));
  let mut db = ProjectDb::open(root).map_err(store)?;
  if !held && actor == HoldActor::Automation && db.queue_holds().map_err(store)?.by_person.contains(&scope) {
    return Err(HoldRefused::PersonOnly);
  }
  db.set_queue_hold((scope, actor), held, Timestamp::now()).map_err(store)
}
