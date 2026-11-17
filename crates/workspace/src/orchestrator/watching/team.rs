//! A linked project's team tasks that never ran here, as what a conversation watches lists them. The team server owns a
//! team task's definition and this computer keeps no row of it (`crate::tasks::recorded`): one that ran here is read
//! with the project's tasks as its runs recorded it, with what waits in its latest run
//! (`crate::tasks::data::ProjectTasks::read_with_recorded`), and one that never ran here as the team server's last
//! reading of what is watched holds it ([`TeamReading`]): its key, its title and the module it is in. Local and team
//! projects list what they lead the same way (`docs/design/local-and-team-parity.md`).

use std::collections::HashSet;

use orchester_core::{
  id::TaskId,
  name::Name,
  task::{Task, TaskDetails, TaskLimits},
  time::Timestamp,
};

use crate::orchestrator::team_status::{TeamReading, TeamTask};

/// The team tasks `team` saw that `read`, the project's tasks as read, do not hold: those that never ran on this
/// computer, drafts here with nothing under way.
pub(super) fn unrun(read: &[Task], team: &TeamReading) -> Vec<Task> {
  let known: HashSet<TaskId> = read.iter().map(Task::id).collect();
  team
    .seen()
    .into_iter()
    .filter(|(id, _)| !known.contains(id))
    .filter_map(|(id, seen)| seen_task(id, seen))
    .collect()
}

/// Team task `id` as the team server's last reading `seen` it, for a task that never ran on this computer: its key,
/// its title and its module. `None` for a task with neither a title nor a key the app could have saved.
pub(super) fn seen_task(id: TaskId, seen: &TeamTask) -> Option<Task> {
  // A title the app could not have saved is named by the task's key.
  let title = Name::new(&seen.title)
    .or_else(|_| Name::new(&seen.key.to_string()))
    .ok()?;
  let details = TaskDetails {
    title,
    brief: String::new(),
    acceptance_criteria: Vec::new(),
    pipeline: None,
    limits: TaskLimits::NONE,
  };
  // When the server made it is not read: nothing here shows it.
  Some(Task::draft(id, seen.key.clone(), details, Timestamp::from_unix_millis(0)).with_module(seen.module))
}
