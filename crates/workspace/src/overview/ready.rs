//! *Ready to start* on the quiet Overview (02 `HWznK`): the drafts a person could queue now.

use orchester_core::task::{Task, TaskStatus};

use crate::tasks::data::ProjectTasks;

/// The drafts of `tasks`, in their order, that could be queued now: open, without a run, missing no detail, and
/// blocked by no task.
pub fn ready_to_start(tasks: &ProjectTasks) -> Vec<&Task> {
  tasks
    .tasks
    .iter()
    .filter(|task| {
      task.status() == TaskStatus::Draft
        && !task.organization().status().is_closed()
        && !tasks.phases.contains_key(&task.id())
        && task.details().missing().is_empty()
        && tasks.blocking.get(&task.id()).is_none_or(Vec::is_empty)
    })
    .collect()
}
