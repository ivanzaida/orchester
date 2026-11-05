//! A task the leading conversation creates outside what it watches becomes a task subscription of it, so the end the
//! conversation waits for comes back to it. The new subscription takes the order of the subscription whose turn
//! created the task; a turn the person started has none, so it takes the default order.

use super::{Watch, WatchTarget};
use crate::id::{ModuleId, TaskId};

/// The task subscription the conversation with `watches` gets for the task `task` (in `module`, or in none) one of its
/// turns created: `None` when a subscription of the conversation already takes the task in. `turn_of` is the
/// subscription whose event turn created it, `None` for a turn of the person's.
pub fn created_task_watch(
  watches: &[Watch],
  turn_of: Option<&Watch>,
  task: TaskId,
  module: Option<ModuleId>,
) -> Option<Watch> {
  if watches.iter().any(|watch| watch.target.covers(task, module)) {
    return None;
  }
  Some(Watch {
    target: WatchTarget::Task(task),
    order: turn_of.and_then(|watch| watch.order.clone()),
  })
}
