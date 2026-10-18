//! The background operations of a task's runs: merging one, removing its worktrees and opening a pull request
//! from it, as [`super::operations`] describes the others.

use orchester_core::id::TaskId;

use super::{
  log::{OperationSpec, Place, Target},
  operations::in_project,
};
use crate::{messages::Message, workspace::OpenProject};

/// Merging run `number` of task `key` ("ORC-12", 3) into `branch`, then clearing what it left.
pub fn run_merge(open: &OpenProject, task: TaskId, (key, number): (&str, u32), branch: &str) -> OperationSpec {
  let message = |text| {
    Message::new(text)
      .with("task", key.to_owned())
      .with("run", number.to_string())
      .with("branch", branch.to_owned())
  };
  OperationSpec::new(
    message("statusBar.operations.runMerge.running"),
    message("statusBar.operations.runMerge.done"),
    message("statusBar.operations.runMerge.failed"),
  )
  .project(open.project.name().as_str())
  .target(in_project(open, Place::Task(task)))
}

/// What a merge of run `number` of task `key` records beside its own event when the temporary
/// worktree of its merge commit stayed at `path`: the project, the words and the task.
pub fn run_merge_left(
  open: &OpenProject,
  task: TaskId,
  (key, number): (&str, u32),
  path: &str,
) -> (std::sync::Arc<str>, Message, Target) {
  let title = Message::new("statusBar.operations.runMerge.temporaryLeft")
    .with("task", key.to_owned())
    .with("run", number.to_string())
    .with("path", path.to_owned());
  (
    open.project.name().as_str().into(),
    title,
    in_project(open, Place::Task(task)),
  )
}

/// Removing the worktrees of run `number` of task `key` ("ORC-12", 3).
pub fn run_cleanup(open: &OpenProject, task: TaskId, (key, number): (&str, u32)) -> OperationSpec {
  let message = |text| {
    Message::new(text)
      .with("task", key.to_owned())
      .with("run", number.to_string())
  };
  OperationSpec::new(
    message("statusBar.operations.runCleanup.running"),
    message("statusBar.operations.runCleanup.done"),
    message("statusBar.operations.runCleanup.failed"),
  )
  .project(open.project.name().as_str())
  .target(in_project(open, Place::Task(task)))
}

/// Pushing the branch of run `number` of task `key` ("ORC-12", 3) and opening a pull request from it.
pub fn run_pull_request(open: &OpenProject, task: TaskId, (key, number): (&str, u32)) -> OperationSpec {
  let message = |text| {
    Message::new(text)
      .with("task", key.to_owned())
      .with("run", number.to_string())
  };
  OperationSpec::new(
    message("statusBar.operations.runPullRequest.running"),
    message("statusBar.operations.runPullRequest.done"),
    message("statusBar.operations.runPullRequest.failed"),
  )
  .project(open.project.name().as_str())
  .target(in_project(open, Place::Task(task)))
}
