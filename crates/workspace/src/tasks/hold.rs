//! A person's hold on new starts (`docs/design/orchester-workspace.md`, "Pause scheduling"): in the whole project or
//! in one reviewed group, kept in the project's database, where the runner reads it before it starts a first run.
//! Only new starts wait: running runs go on, and so do resumes and runs again. The Tasks screen, Orchester's MCP
//! server and the Orchestrator chat hold and release the queue through [`hold_in`], with the same rules.

use std::path::Path;

use orchester_core::{
  id::{IdempotencyKey, TaskId},
  store::TaskStore,
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
