//! Story points estimated by a project's runner ([`Command::EstimateTask`]), on auto mode's reviewer's connection
//! and model, whose API key only the runner reads.

use std::future::Future;

use orchester_core::{id::TaskId, task::estimate::EstimateOutcome};
use orchester_ipc::{Command, Reply};

use super::{RequestError, RunRequestError, Runners};
use crate::workspace::OpenProject;

impl Runners {
  /// Asks `project`'s runner, starting it if needed, to estimate the story points of `task`.
  pub async fn estimate_task_in(
    &self,
    project: &OpenProject,
    task: TaskId,
  ) -> Result<EstimateOutcome, RunRequestError> {
    match self.ask_project(project, Command::EstimateTask { task }).await? {
      Reply::TaskEstimated { outcome, .. } => Ok(outcome),
      Reply::Rejected(rejection) => Err(RunRequestError::Rejected(rejection)),
      other => {
        tracing::error!(?other, "unexpected reply to EstimateTask");
        Err(RunRequestError::Request(RequestError::Lost))
      }
    }
  }

  /// Runs `work` on the runners' runtime, without waiting for it.
  pub(crate) fn spawn(&self, work: impl Future<Output = ()> + Send + 'static) {
    self.inner.runtime.spawn(work);
  }
}
