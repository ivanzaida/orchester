//! What every part of a task's page reads: the task as last read and where it is, the same for the
//! header, the Summary and the tabs and slots other tasks build (the Conversation, Changes, and the
//! Summary's decisions), so each of them mounts with one line.

use std::sync::Arc;

use orchester_core::project::TaskKey;
use orchester_workspace::tasks::{approvals::ShownApproval, detail::TaskDetail, integration_jobs::ProjectJobState};

use super::read::TaskLeftovers;
use crate::project::{ShownProject, TasksState};

#[derive(Clone, PartialEq)]
pub struct TaskContext {
  /// The open project with its feed.
  pub project: ShownProject,
  /// The task's key, as its page's path names it.
  pub key: TaskKey,
  pub detail: Arc<TaskDetail>,
  /// What the task's runs without live work left in the repository.
  pub left: TaskLeftovers,
  /// The project's runner is online: a run recorded as running without one is interrupted.
  pub online: bool,
  /// The project's merge and removal operations as they stand.
  pub jobs: ProjectJobState,
}

impl lurq::app::component::DevtoolsInspectable for TaskContext {}

impl TaskContext {
  /// The page of this task on `view`, at the address of a task kept *Not shared* when it is one.
  pub fn page(&self, view: crate::routes::TaskView) -> crate::routes::ProjectPage {
    crate::routes::ProjectPage::task(self.detail.task.key().clone(), view, self.detail.not_shared())
  }

  /// The run is recorded as running while no runner looks after it: it is interrupted until one does.
  pub fn lost(&self) -> bool {
    self.detail.lost_its_runner(self.online)
  }

  /// A tool request of the task's latest run that waits for the person, the oldest first, as the
  /// project's feed reads the runner's pending requests. A run without a runner waits for no one:
  /// nothing answers its requests until a runner is back. Reading it in a render re-renders it when the
  /// project's tasks are read again.
  pub fn waiting_approval(&self) -> Option<ShownApproval> {
    if self.lost() {
      return None;
    }
    let run = self.detail.run.as_ref()?.id();
    match self.project.feed.tasks.get() {
      TasksState::Ready(tasks) => tasks
        .pending_tool_approvals
        .iter()
        .find(|shown| shown.approval.run == run)
        .cloned(),
      TasksState::Loading | TasksState::Failed(_) => None,
    }
  }
}
