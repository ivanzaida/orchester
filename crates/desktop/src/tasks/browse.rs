//! Task-list query, ordering and selection, independent of rendering.
use std::collections::BTreeSet;

use orchester_core::{
  id::{ResourceId, TaskId},
  run::TaskPhase,
  task_group::GroupHold,
};

use super::{
  data::ProjectTasks,
  group::hold_messages,
  list::{self, TaskRow, TaskTab},
};
use crate::messages::Message;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TaskSort {
  #[default]
  Manual,
  Newest,
  Title,
}
impl lurq::app::component::DevtoolsInspectable for TaskSort {}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PipelineFilter(pub Option<ResourceId>);
impl lurq::app::component::DevtoolsInspectable for PipelineFilter {}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Selection(pub BTreeSet<TaskId>);
impl lurq::app::component::DevtoolsInspectable for Selection {}
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

pub fn rows(data: &ProjectTasks, tab: TaskTab, query: &str, pipeline: PipelineFilter, sort: TaskSort) -> Vec<TaskRow> {
  let query = query.trim().to_lowercase();
  let mut tasks: Vec<_> = data
    .tasks
    .iter()
    .filter(|task| {
      (pipeline.0.is_none() || task.details().pipeline == pipeline.0)
        && (query.is_empty()
          || task.details().title.as_str().to_lowercase().contains(&query)
          || task.key().to_string().to_lowercase().contains(&query)
          || task.id().to_string().to_lowercase().contains(&query)
          || task.details().brief.to_lowercase().contains(&query))
    })
    .cloned()
    .collect();
  match sort {
    TaskSort::Manual => {}
    TaskSort::Newest => tasks.reverse(),
    TaskSort::Title => tasks.sort_by_key(|task| task.details().title.as_str().to_lowercase()),
  }
  list::rows(&tasks, &data.pipelines, &data.phases, tab)
    .into_iter()
    .map(|mut row| {
      match (data.group_holds.get(&row.id), row.phase) {
        (Some(hold), Some(TaskPhase::Queued)) => {
          if let Some(reason) = hold_messages(hold).into_iter().next() {
            row.detail = reason;
          }
        }
        (Some(GroupHold::GroupCap), Some(TaskPhase::StoppedAtLimit)) => {
          row.detail = Message::new("tasks.bulk.stoppedGroup");
        }
        _ => {}
      }
      row
    })
    .collect()
}
