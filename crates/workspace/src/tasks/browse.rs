//! Task-list query, ordering and selection, independent of rendering.
use std::collections::BTreeSet;

use orchester_core::{
  id::{ResourceId, TaskId},
  run::TaskPhase,
  task::{
    Task,
    organization::{self, TaskFilter, WorkflowStatus},
  },
  task_group::GroupHold,
};

use super::{
  data::ProjectTasks,
  group::hold_row_message,
  list::{self, ReadinessKind, TaskRow, TaskTab},
};
use crate::messages::Message;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TaskSort {
  #[default]
  Manual,
  Newest,
  Title,
  /// Workflow order, then the most recently changed first (`rVJoA`).
  Status,
  /// Urgent first, then the most recently changed first (`rVJoA`).
  Priority,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PipelineFilter(pub Option<ResourceId>);

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Selection(pub BTreeSet<TaskId>);
impl Selection {
  pub fn toggle(&mut self, id: TaskId) {
    if !self.0.remove(&id) {
      self.0.insert(id);
    }
  }
  /// Select all only affects the visible query. Hidden selections remain explicit in the count.
  pub fn toggle_all(&mut self, visible: &[TaskRow]) {
    let all = visible.iter().all(|row| self.0.contains(&row.id));
    for row in visible {
      if all {
        self.0.remove(&row.id);
      } else {
        self.0.insert(row.id);
      }
    }
  }
}

/// What narrows the task list: the search, the pipeline and the organization filters.
#[derive(Clone, Copy)]
pub struct Query<'a> {
  pub text: &'a str,
  pub pipeline: PipelineFilter,
  pub organization: &'a TaskFilter,
  /// Joins the keys of a row's blocking tasks (`common.listSeparator`).
  pub separator: &'a str,
}

pub fn rows(data: &ProjectTasks, tab: TaskTab, query: Query<'_>, sort: TaskSort) -> Vec<TaskRow> {
  let text = query.text.trim().to_lowercase();
  let mut tasks: Vec<_> = data
    .tasks
    .iter()
    .filter(|task| {
      (query.pipeline.0.is_none() || task.details().pipeline == query.pipeline.0)
        && query.organization.includes(task.organization())
        && matches_text(task, data.former_keys.get(&task.id()), &text)
    })
    .cloned()
    .collect();
  match sort {
    TaskSort::Manual => {}
    TaskSort::Newest => tasks.reverse(),
    TaskSort::Title => tasks.sort_by_key(|task| task.details().title.as_str().to_lowercase()),
    TaskSort::Status => sort_organized(data, &mut tasks, organization::TaskSort::Status),
    TaskSort::Priority => sort_organized(data, &mut tasks, organization::TaskSort::Priority),
  }
  list::rows(&tasks, data, tab)
    .into_iter()
    .map(|row| {
      let mut row = data.decorate_row(row, query.separator);
      // A blocked task says what blocks it rather than how its group waits.
      if data.blocking.get(&row.id).is_some_and(|by| !by.is_empty()) {
        return row;
      }
      // A Canceled task its group will not start says so rather than how the group waits.
      if row.phase == Some(TaskPhase::Queued)
        && row.organization.status() == WorkflowStatus::Canceled
        && data.grouped.contains(&row.id)
      {
        row.detail = hold_row_message(&GroupHold::Canceled);
        row.readiness = ReadinessKind::Held(GroupHold::Canceled);
        return row;
      }
      match (data.group_holds.get(&row.id), row.phase) {
        (Some(hold), Some(TaskPhase::Queued)) => {
          row.detail = hold_row_message(hold);
          row.readiness = ReadinessKind::Held(hold.clone());
        }
        (Some(GroupHold::GroupCap), Some(TaskPhase::StoppedAtLimit)) => {
          row.detail = Message::new("tasks.bulk.stoppedGroup");
        }
        // New starts held in the whole project hold every queued task, in a group or not.
        (None, Some(TaskPhase::Queued)) if data.queue.project => {
          row.detail = hold_row_message(&GroupHold::QueueHeld);
          row.readiness = ReadinessKind::Held(GroupHold::QueueHeld);
        }
        _ => {}
      }
      row
    })
    .collect()
}

/// The search matches the title, key, former key, id or brief, ignoring case; an empty search
/// matches all.
fn matches_text(task: &Task, former: Option<&String>, text: &str) -> bool {
  text.is_empty()
    || task.details().title.as_str().to_lowercase().contains(text)
    || former.is_some_and(|key| key.to_lowercase().contains(text))
    || task.key().to_string().to_lowercase().contains(text)
    || task.id().to_string().to_lowercase().contains(text)
    || task.details().brief.to_lowercase().contains(text)
}

/// Orders by workflow status or priority, the most recently changed first within each.
fn sort_organized(data: &ProjectTasks, tasks: &mut [Task], sort: organization::TaskSort) {
  let key = |task: &Task| {
    let updated = data.updated.get(&task.id()).copied().unwrap_or(task.created_at());
    (task.organization().clone(), updated, task.id())
  };
  tasks.sort_by(|left, right| {
    let (left, right) = (key(left), key(right));
    sort.compare((&left.0, left.1, left.2), (&right.0, right.1, right.2))
  });
}
