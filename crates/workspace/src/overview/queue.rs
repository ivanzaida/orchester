//! *Queue*: the queued tasks in start order (the order the project lists them, as the daily report's queued tasks),
//! each with why it waits in the app's words (`tasks.bulk.held.*`), and its place in line. Drafts, blocked or not,
//! stay in Tasks.

use std::sync::Arc;

use orchester_core::{
  id::TaskId,
  run::TaskPhase,
  task::{Task, TaskStatus},
  task_group::GroupHold,
};

use crate::{
  messages::Message,
  tasks::{data::ProjectTasks, group::hold_row_message},
};

/// Why a queued task waits.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum QueueReason {
  /// Every agent slot is in use.
  Slot,
  /// For these tasks to finish, by key.
  Blocked(Vec<Arc<str>>),
  /// For the task before it in its reviewed group, by key.
  Previous(Arc<str>),
  /// Its reviewed group holds it, as the hold says.
  Group(GroupHold),
  /// A person held new starts in the project.
  ProjectHeld,
  /// Nothing holds it: the runner starts it in its turn.
  Queued,
}

impl QueueReason {
  /// The reason in words, keys joined by `separator`.
  pub fn message(&self, separator: &str) -> Message {
    match self {
      Self::Slot => Message::new("overview.queue.slot"),
      Self::Blocked(keys) => Message::new("overview.queue.blocked").with("tasks", keys.join(separator)),
      Self::Previous(key) => Message::new("overview.queue.previous").with("task", key.to_string()),
      Self::Group(hold) => hold_row_message(hold),
      Self::ProjectHeld => Message::new("overview.queue.held"),
      Self::Queued => Message::new("overview.queue.queued"),
    }
  }
}

/// A queued task, its reason and its place in line, 1 first.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QueuedLine {
  pub task: TaskId,
  pub key: Arc<str>,
  pub title: Arc<str>,
  pub reason: QueueReason,
  pub place: usize,
}

/// The queued tasks of `tasks`; `slots_full`: the runner reported every agent slot in use.
pub fn queue(tasks: &ProjectTasks, slots_full: bool) -> Vec<QueuedLine> {
  tasks
    .tasks
    .iter()
    .filter(|task| queued(tasks, task))
    .enumerate()
    .map(|(index, task)| QueuedLine {
      task: task.id(),
      key: task.key().to_string().into(),
      title: task.details().title.as_str().into(),
      reason: reason(tasks, task, slots_full),
      place: index + 1,
    })
    .collect()
}

/// Whether `task` waits in the queue for its run to start; a closed one waits for nothing.
fn queued(tasks: &ProjectTasks, task: &Task) -> bool {
  if task.organization().status().is_closed() {
    return false;
  }
  match tasks.phases.get(&task.id()) {
    None => task.status() == TaskStatus::Queued,
    Some(latest) => latest.phase() == Some(TaskPhase::Queued),
  }
}

fn reason(tasks: &ProjectTasks, task: &Task, slots_full: bool) -> QueueReason {
  let key = |id: &TaskId| -> Option<Arc<str>> {
    let task = tasks.tasks.iter().find(|task| task.id() == *id)?;
    Some(task.key().to_string().into())
  };
  let keys = |ids: &[TaskId]| ids.iter().filter_map(key).collect::<Vec<_>>();
  if let Some(blocking) = tasks.blocking.get(&task.id()).filter(|links| !links.is_empty()) {
    return QueueReason::Blocked(keys(blocking));
  }
  match tasks.group_holds.get(&task.id()) {
    Some(GroupHold::AgentCapacity) => return QueueReason::Slot,
    Some(GroupHold::Blocked { by }) => return QueueReason::Blocked(keys(by)),
    Some(hold @ GroupHold::PreviousTask) => {
      return tasks
        .run_facts
        .previous_in_group
        .get(&task.id())
        .and_then(key)
        .map_or_else(|| QueueReason::Group(hold.clone()), QueueReason::Previous);
    }
    Some(hold) => return QueueReason::Group(hold.clone()),
    None => {}
  }
  if tasks.queue.project {
    QueueReason::ProjectHeld
  } else if slots_full {
    QueueReason::Slot
  } else {
    QueueReason::Queued
  }
}

/// A place in line: *next*, then *2nd*, *3rd*, *4th*. lurq has no ordinal rules, so each ending is a key.
pub fn place(place: usize) -> Message {
  if place <= 1 {
    return Message::new("overview.queue.place.next");
  }
  let key = match (place % 10, place % 100) {
    (_, 11..=13) => "overview.queue.place.th",
    (1, _) => "overview.queue.place.st",
    (2, _) => "overview.queue.place.nd",
    (3, _) => "overview.queue.place.rd",
    _ => "overview.queue.place.th",
  };
  Message::new(key).with("place", place.to_string())
}
