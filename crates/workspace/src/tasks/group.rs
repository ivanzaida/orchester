//! Review validation reuses single-task preflight, then saves one atomic queue group.
use std::{collections::BTreeSet, path::Path};

use orchester_core::{
  id::{ResourceId, TaskId},
  resource::Pipeline,
  store::{ResourceStore, TaskStore},
  task::TaskStatus,
  task_group::{GroupHold, ReviewedPipeline, TaskGroup},
  task_selection::SelectionProblem,
};
use orchester_store::ProjectDb;

use crate::{
  messages::{self, Message},
  tasks::{
    data::ProjectTasks,
    list::missing_key,
    run::{check, preflight_message},
  },
};

pub fn problem_message(problem: &SelectionProblem) -> Message {
  match problem {
    SelectionProblem::NotDraft => Message::new("tasks.bulk.notDraft"),
    SelectionProblem::HasRun => Message::new("tasks.bulk.hasRun"),
    SelectionProblem::Missing(detail) => Message::new(missing_key(*detail)),
    SelectionProblem::PipelineNotPublished => Message::new("runs.preflight.pipelineNotPublished"),
    SelectionProblem::Blocked { .. } => Message::new("tasks.bulk.blocked"),
  }
}

pub fn hold_messages(hold: &orchester_core::task_group::GroupHold) -> Vec<Message> {
  use orchester_core::task_group::GroupHold;
  let key = match hold {
    GroupHold::SessionEnded => "tasks.bulk.hold.sessionEnded",
    GroupHold::AgentCapacity => "tasks.bulk.hold.capacity",
    GroupHold::DailyBudget => "runs.capacity.dailyBudgetUsed",
    GroupHold::GroupCap => "tasks.bulk.hold.groupCap",
    GroupHold::PreviousTask => "tasks.bulk.hold.previous",
    GroupHold::Concurrency => "tasks.bulk.hold.concurrency",
    GroupHold::FailedTask => "tasks.bulk.hold.failure",
    GroupHold::StartFailed => "tasks.bulk.hold.startFailed",
    GroupHold::Blocked { .. } => "tasks.bulk.hold.blocked",
    GroupHold::Canceled => "tasks.bulk.hold.canceled",
    GroupHold::QueueHeld => "tasks.bulk.hold.queueHeld",
    GroupHold::Paused => "tasks.bulk.hold.paused",
    GroupHold::Preflight(problems) => return problems.iter().map(preflight_message).collect(),
  };
  vec![Message::new(key)]
}

/// The hold in a few words, for a task row's readiness cell; [`hold_messages`] says it in full.
pub const fn hold_row_message(hold: &GroupHold) -> Message {
  Message::new(match hold {
    GroupHold::SessionEnded => "tasks.bulk.held.sessionEnded",
    GroupHold::AgentCapacity => "tasks.bulk.held.capacity",
    GroupHold::DailyBudget => "tasks.bulk.held.dailyBudget",
    GroupHold::GroupCap => "tasks.bulk.held.groupCap",
    GroupHold::PreviousTask => "tasks.bulk.held.previous",
    GroupHold::Concurrency => "tasks.bulk.held.concurrency",
    GroupHold::FailedTask => "tasks.bulk.held.failure",
    GroupHold::Preflight(_) => "tasks.bulk.held.preflight",
    GroupHold::StartFailed => "tasks.bulk.held.startFailed",
    GroupHold::Blocked { .. } => "tasks.bulk.held.blocked",
    GroupHold::Canceled => "tasks.bulk.held.canceled",
    GroupHold::QueueHeld => "tasks.bulk.held.queueHeld",
    GroupHold::Paused => "tasks.bulk.held.paused",
  })
}

/// The keys of the problem line for selected tasks a group holds, for one task and for several:
/// *2 tasks held after a task failed · Return to drafts* (02.03D `IYlGK`).
pub const fn held_line_keys(hold: &GroupHold) -> (&'static str, &'static str) {
  match hold {
    GroupHold::SessionEnded => (
      "tasks.bulk.heldLine.sessionEnded.one",
      "tasks.bulk.heldLine.sessionEnded.other",
    ),
    GroupHold::AgentCapacity => ("tasks.bulk.heldLine.capacity.one", "tasks.bulk.heldLine.capacity.other"),
    GroupHold::DailyBudget => (
      "tasks.bulk.heldLine.dailyBudget.one",
      "tasks.bulk.heldLine.dailyBudget.other",
    ),
    GroupHold::GroupCap => ("tasks.bulk.heldLine.groupCap.one", "tasks.bulk.heldLine.groupCap.other"),
    GroupHold::PreviousTask => ("tasks.bulk.heldLine.previous.one", "tasks.bulk.heldLine.previous.other"),
    GroupHold::Concurrency => (
      "tasks.bulk.heldLine.concurrency.one",
      "tasks.bulk.heldLine.concurrency.other",
    ),
    GroupHold::FailedTask => ("tasks.bulk.heldLine.failure.one", "tasks.bulk.heldLine.failure.other"),
    GroupHold::Preflight(_) => (
      "tasks.bulk.heldLine.preflight.one",
      "tasks.bulk.heldLine.preflight.other",
    ),
    GroupHold::StartFailed => (
      "tasks.bulk.heldLine.startFailed.one",
      "tasks.bulk.heldLine.startFailed.other",
    ),
    GroupHold::Blocked { .. } => ("tasks.bulk.heldLine.blocked.one", "tasks.bulk.heldLine.blocked.other"),
    GroupHold::Canceled => ("tasks.bulk.heldLine.canceled.one", "tasks.bulk.heldLine.canceled.other"),
    GroupHold::QueueHeld => (
      "tasks.bulk.heldLine.queueHeld.one",
      "tasks.bulk.heldLine.queueHeld.other",
    ),
    GroupHold::Paused => ("tasks.bulk.heldLine.paused.one", "tasks.bulk.heldLine.paused.other"),
  }
}

pub fn assign(root: &Path, tasks: &[TaskId], pipeline: ResourceId) -> Result<Vec<Message>, Message> {
  let mut db = ProjectDb::open(root).map_err(|error| messages::store_error(&error))?;
  let skipped = db
    .set_tasks_pipeline(tasks, pipeline)
    .map_err(|error| messages::store_error(&error))?;
  skipped
    .iter()
    .map(|(id, reason)| {
      let task = db
        .task(*id)
        .map_err(|error| messages::store_error(&error))?
        .ok_or_else(|| Message::new("runs.start.taskMissing"))?;
      let key = match reason {
        SelectionProblem::HasRun => "tasks.bulk.skippedHasRun",
        _ => "tasks.bulk.skippedNotDraft",
      };
      Ok(Message::new(key).with("task", task.key().to_string()))
    })
    .collect()
}

/// Takes held groups' unstarted `tasks` back to the drafts ([`ProjectDb::release_held_tasks`]).
pub fn release(root: &Path, tasks: &[TaskId]) -> Result<(), Message> {
  let mut db = ProjectDb::open(root).map_err(|error| messages::store_error(&error))?;
  db.release_held_tasks(tasks)
    .map_err(|error| messages::store_error(&error))
}

/// The selected tasks a person may take back to the drafts: queued, never started, in a group
/// held for good ([`GroupHold::releases_unstarted`]).
pub fn releasable(tasks: &ProjectTasks, selected: &BTreeSet<TaskId>) -> Vec<TaskId> {
  tasks
    .tasks
    .iter()
    .filter(|task| {
      selected.contains(&task.id())
        && task.status() == TaskStatus::Queued
        && !tasks.phases.contains_key(&task.id())
        && tasks
          .group_holds
          .get(&task.id())
          .is_some_and(GroupHold::releases_unstarted)
    })
    .map(|task| task.id())
    .collect()
}

pub fn queue(
  root: &Path,
  group: &TaskGroup,
  limits: orchester_core::task::TaskLimits,
  reviewed: &[ReviewedPipeline],
) -> Result<TaskGroup, Vec<Message>> {
  let mut db = ProjectDb::open(root).map_err(|error| vec![messages::store_error(&error)])?;
  if let Some(existing) = db
    .task_group(group.key())
    .map_err(|error| vec![messages::store_error(&error)])?
  {
    return Ok(existing);
  }
  let mut problems = Vec::new();
  for id in group.tasks() {
    let task = db
      .task(*id)
      .map_err(|error| vec![messages::store_error(&error)])?
      .ok_or_else(|| vec![Message::new("runs.start.taskMissing")])?;
    let pipeline = task
      .details()
      .pipeline
      .ok_or_else(|| vec![Message::new("runs.preflight.pipelineNotPublished")])?;
    let reviewed = reviewed
      .iter()
      .find(|entry| entry.task == *id && entry.pipeline == pipeline)
      .ok_or_else(|| vec![Message::new("tasks.bulk.reviewChanged")])?;
    let version = db
      .version::<Pipeline>(pipeline, reviewed.version)
      .map_err(|error| vec![messages::store_error(&error)])?
      .ok_or_else(|| vec![Message::new("runs.preflight.pipelineNotPublished")])?;
    problems.extend(
      check(&db, task.details(), version.number())
        .map_err(|error| vec![messages::store_error(&error)])?
        .iter()
        .map(preflight_message),
    );
  }
  if !problems.is_empty() {
    return Err(problems);
  }
  db.queue_task_group(group, Some(limits), reviewed)
    .map_err(|error| vec![messages::store_error(&error)])
}
