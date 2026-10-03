//! Readiness of drafts selected for their first run and draft-only pipeline assignment.

use crate::{
  id::{ResourceId, TaskId},
  task::{MissingDetail, Task, TaskStatus},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SelectionProblem {
  NotDraft,
  HasRun,
  Missing(MissingDetail),
  PipelineNotPublished,
  /// Linked blocking tasks have not finished: the runner would refuse to start it.
  Blocked {
    by: Vec<TaskId>,
  },
}

/// All reasons this task cannot enter a first-run group. A persisted run always wins over status.
/// `blocked_by` are its linked blocking tasks that have not finished.
pub fn problems(task: &Task, has_run: bool, published: &[ResourceId], blocked_by: &[TaskId]) -> Vec<SelectionProblem> {
  let mut problems = Vec::new();
  if has_run {
    problems.push(SelectionProblem::HasRun);
  } else if task.status() != TaskStatus::Draft {
    problems.push(SelectionProblem::NotDraft);
  }
  problems.extend(task.details().missing().into_iter().map(SelectionProblem::Missing));
  if let Some(pipeline) = task.details().pipeline
    && !published.contains(&pipeline)
  {
    problems.push(SelectionProblem::PipelineNotPublished);
  }
  if !blocked_by.is_empty() {
    problems.push(SelectionProblem::Blocked {
      by: blocked_by.to_vec(),
    });
  }
  problems
}

/// Pipeline assignment never changes queued tasks or tasks that already have runs.
pub fn assign_pipeline(task: &mut Task, has_run: bool, pipeline: ResourceId) -> Result<(), SelectionProblem> {
  if has_run {
    return Err(SelectionProblem::HasRun);
  }
  if task.status() != TaskStatus::Draft {
    return Err(SelectionProblem::NotDraft);
  }
  let mut details = task.details().clone();
  details.pipeline = Some(pipeline);
  task
    .edit(details, false)
    .expect("a draft without runs accepts incomplete details");
  Ok(())
}
