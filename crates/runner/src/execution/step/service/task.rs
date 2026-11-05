//! The service tools a run's invocation gets.

use std::{path::Path, sync::Arc};

use orchester_core::{
  connection::stream::RunnerProblem,
  id::InvocationId,
  resource::CommandEnvironment,
  service::{ServiceName, StackName},
};

use super::{
  StepServices,
  launch::{published, published_stacks},
};
use crate::execution::{run::RunTask, step::start::problem};

impl RunTask {
  /// The names of the project's published services, which the step's service tools take.
  pub(in crate::execution::step) fn service_names(&self) -> Result<Vec<ServiceName>, RunnerProblem> {
    let services =
      published(&self.root).map_err(|error| problem(format!("cannot read the project's services: {error}")))?;
    Ok(services.iter().map(|service| service.name().clone()).collect())
  }

  /// The names of the project's published stacks, which `start_stack` takes.
  pub(in crate::execution::step) fn stack_names(&self) -> Result<Vec<StackName>, RunnerProblem> {
    let stacks =
      published_stacks(&self.root).map_err(|error| problem(format!("cannot read the project's stacks: {error}")))?;
    Ok(stacks.iter().map(|stack| stack.name().clone()).collect())
  }

  /// The service tools of `invocation`, whose agent works in `worktree` with the project's `environment`.
  pub(in crate::execution::step) fn step_services(
    &self,
    invocation: InvocationId,
    worktree: &Path,
    environment: &CommandEnvironment,
  ) -> StepServices {
    StepServices {
      services: Arc::clone(&self.services),
      state: self.services.register(invocation),
      root: self.root.clone(),
      project_id: self.project.id(),
      worktree: worktree.to_path_buf(),
      run: self.run.id(),
      vault: Arc::clone(&self.vault),
      inherited: std::env::vars_os().collect(),
      project: environment.clone(),
    }
  }
}
