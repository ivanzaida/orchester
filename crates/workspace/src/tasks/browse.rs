//! Task-list query, ordering and selection, independent of rendering.
use std::collections::BTreeSet;

use orchester_core::{
  id::{ModuleId, ResourceId, TagId, TaskId},
  run::TaskPhase,
  task::{
    Task,
    organization::{self, Priority, TaskFilter},
  },
  task_group::GroupHold,
};

use super::{
  data::ProjectTasks,
  group::hold_row_message,
  list::{self, ReadinessKind, TaskRow, TaskTab},
};
use crate::{messages::Message, modules::ModuleFilter};

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
  rows_in(data, tab, (query, ModuleFilter::Any), sort)
}

/// [`rows`] narrowed to the tasks of a module, or of none (ORC-48).
pub fn rows_in(
  data: &ProjectTasks,
  tab: TaskTab,
  (query, module): (Query<'_>, ModuleFilter),
  sort: TaskSort,
) -> Vec<TaskRow> {
  let keeps = |task: &Task| {
    (query.pipeline.0.is_none() || task.details().pipeline == query.pipeline.0)
      && module.keeps(task)
      && query.organization.includes(task.organization())
  };
  rows_where(data, tab, (query.text, query.separator), sort, keeps)
}

/// What the Tasks list's Filter menu narrows it to (ORC-99): for each facet the values chosen, any of which a task
/// has; a facet with none chosen keeps every task. `None` among the pipelines or the modules is a task without one.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Facets {
  pub pipelines: Vec<Option<ResourceId>>,
  pub modules: Vec<Option<ModuleId>>,
  pub priorities: Vec<Priority>,
  pub tags: Vec<TagId>,
}

impl Facets {
  /// How many values are chosen, over every facet.
  pub fn count(&self) -> usize {
    self.pipelines.len() + self.modules.len() + self.priorities.len() + self.tags.len()
  }

  pub fn is_empty(&self) -> bool {
    self.count() == 0
  }

  fn keeps(&self, task: &Task, organization: &TaskFilter) -> bool {
    (self.pipelines.is_empty() || self.pipelines.contains(&task.details().pipeline))
      && (self.modules.is_empty() || self.modules.contains(&task.module()))
      && organization.includes(task.organization())
  }
}

/// Every task the search `text` finds among those `facets` keep, as rows ordered by `sort` (ORC-99): the Tasks list
/// splits them into its views ([`super::views`]). `separator` joins the keys of a row's blocking tasks.
pub fn faceted(data: &ProjectTasks, (text, separator): (&str, &str), facets: &Facets, sort: TaskSort) -> Vec<TaskRow> {
  let organization = TaskFilter {
    priorities: facets.priorities.clone(),
    tags: facets.tags.clone(),
    ..TaskFilter::default()
  };
  rows_where(data, TaskTab::All, (text, separator), sort, |task| {
    facets.keeps(task, &organization)
  })
}

/// The rows of `tab` the search `text` finds among the tasks `keeps` keeps, ordered by `sort`, each saying what blocks
/// it or how its group waits.
fn rows_where(
  data: &ProjectTasks,
  tab: TaskTab,
  (text, separator): (&str, &str),
  sort: TaskSort,
  keeps: impl Fn(&Task) -> bool,
) -> Vec<TaskRow> {
  let text = text.trim().to_lowercase();
  let mut tasks: Vec<_> = data
    .tasks
    .iter()
    .filter(|task| keeps(task) && matches_text(task, data.former_keys.get(&task.id()), &text))
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
      let mut row = data.decorate_row(row, separator);
      // A blocked task says what blocks it rather than how its group waits.
      if data.blocking.get(&row.id).is_some_and(|by| !by.is_empty()) {
        return row;
      }
      // A closed task its group will not start says so rather than how the group waits.
      if row.phase == Some(TaskPhase::Queued) && row.organization.status().is_closed() && data.grouped.contains(&row.id)
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
