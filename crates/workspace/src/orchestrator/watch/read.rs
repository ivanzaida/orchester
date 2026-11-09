//! What a watch command reads of the project: whether its target still exists and what it is called now, a
//! conversation's title, and what a stop leaves open.

use std::collections::HashMap;

use orchester_core::{
  id::{ChatId, ModuleId},
  module::ModuleState,
  orchestrator::{
    Subscriptions, WatchTarget,
    watch::{LeftOpen, NamedTarget, OpenTask},
  },
  run::TaskPhase,
  store::{MergeStore, RunStore},
};
use orchester_store::{ProjectDb, StoreError};

use crate::tasks::{
  detail::run_store_error,
  recorded::{self, task_of},
};

/// A target that still exists, named as people read it now, with the module of a task target (`None`: it has none,
/// or the target is no task).
pub(super) struct Found {
  pub(super) named: NamedTarget,
  pub(super) module: Option<ModuleId>,
}

/// `target` as it stands now; `None` when it was deleted, or is a module that was closed.
pub(super) fn find(db: &ProjectDb, target: WatchTarget) -> Result<Option<Found>, StoreError> {
  let found = |name: Option<String>, module| Found {
    named: NamedTarget { target, name },
    module,
  };
  Ok(match target {
    WatchTarget::Task(task) => task_of(db, task)?.map(|task| found(Some(task.key().to_string()), task.module())),
    WatchTarget::Module(module) => db
      .module(module)?
      .filter(|module| module.details().state() == ModuleState::Open)
      .map(|module| found(Some(module.name().as_str().to_owned()), None)),
    WatchTarget::WithoutModule | WatchTarget::Project => Some(found(None, None)),
  })
}

/// `given` named as it is now, or as given when it is gone.
pub(super) fn renamed(db: &ProjectDb, given: &NamedTarget) -> Result<NamedTarget, StoreError> {
  Ok(find(db, given.target)?.map_or_else(|| given.clone(), |found| found.named))
}

/// The title of conversation `chat`, as History shows it; empty for a conversation that is not kept, which leads
/// nothing (its subscriptions end with it).
pub(super) fn title(db: &ProjectDb, chat: ChatId) -> Result<String, StoreError> {
  Ok(db.chat(chat)?.map(|kept| kept.title().to_owned()).unwrap_or_default())
}

/// What stopping `chat`'s subscriptions to `targets` leaves open: the tasks they take in that `chat` leads in `set`,
/// before the stop, whose latest run waits, works, is queued or stopped at a limit. It only reads them.
pub(super) fn left_open(
  db: &ProjectDb,
  chat: ChatId,
  set: &Subscriptions,
  targets: &[WatchTarget],
) -> Result<Vec<OpenTask>, StoreError> {
  let mut keys = HashMap::new();
  let mut phases = Vec::new();
  for task in recorded::all(db)? {
    let (id, module) = (task.id(), task.module());
    if set.leading(id, module) != Some(chat) || !targets.iter().any(|target| target.covers(id, module)) {
      continue;
    }
    let phase = match db.runs_of_task(id).map_err(run_store_error)?.pop() {
      Some(Ok(run)) => TaskPhase::of(&task, Some(&run), db.run_merge(run.id())?.as_ref()),
      // A latest run that cannot be read back is not named: the stop ends the subscription whatever its runs hold.
      Some(Err(_)) => continue,
      None => TaskPhase::of(&task, None, None),
    };
    keys.insert(id, task.key().to_string());
    phases.push((id, phase));
  }
  Ok(
    LeftOpen::after_stop(phases)
      .tasks()
      .iter()
      .filter_map(|(id, work)| keys.remove(id).map(|task| OpenTask { task, work: *work }))
      .collect(),
  )
}
