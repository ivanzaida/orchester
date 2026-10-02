//! Readiness of drafts selected for their first run and draft-only pipeline assignment.

use crate::{
  id::ResourceId,
  task::{MissingDetail, Task, TaskStatus},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SelectionProblem {
  NotDraft,
  HasRun,
  Missing(MissingDetail),
  PipelineNotPublished,
}

/// All reasons this task cannot enter a first-run group. A persisted run always wins over status.
pub fn problems(task: &Task, has_run: bool, published: &[ResourceId]) -> Vec<SelectionProblem> {
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
