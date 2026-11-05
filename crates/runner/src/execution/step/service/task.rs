//! The service tools a run's invocation gets, and where they record a crash: in the invocation's transcript, as a
//! runner note the run's conversation shows.

use std::{path::Path, sync::Arc};

use orchester_core::{
  connection::stream::RunnerProblem,
  id::InvocationId,
  resource::CommandEnvironment,
  service::{ServiceName, StackName, UiDriver},
};

use super::{
  StepServices,
  launch::{published, published_stacks},
};
use crate::execution::{
  run::RunTask,
  step::{runner_note, start::problem},
};

/// The project's published services as the step's tools are offered them.
pub(in crate::execution::step) struct ServiceNames {
  /// Their names, which the step's service tools take.
  pub(in crate::execution::step) names: Vec<ServiceName>,
  /// Whether one of them has the `native` UI driver, which the `ui_*` tools drive.
  pub(in crate::execution::step) native_ui: bool,
}

impl RunTask {
  /// The project's published services, by name.
  pub(in crate::execution::step) fn service_names(&self) -> Result<ServiceNames, RunnerProblem> {
    let services =
      published(&self.root).map_err(|error| problem(format!("cannot read the project's services: {error}")))?;
    Ok(ServiceNames {
      names: services.iter().map(|service| service.name().clone()).collect(),
      native_ui: services.iter().any(|service| service.ui_driver() == UiDriver::Native),
    })
  }

  /// The names of the project's published stacks, which `start_stack` takes.
  pub(in crate::execution::step) fn stack_names(&self) -> Result<Vec<StackName>, RunnerProblem> {
    let stacks =
      published_stacks(&self.root).map_err(|error| problem(format!("cannot read the project's stacks: {error}")))?;
    Ok(stacks.iter().map(|stack| stack.name().clone()).collect())
  }

  /// The service tools of `invocation`, whose agent works in `worktree` with the project's `environment`; the
  /// Services tab reaches the invocation's services through them until it ends.
  pub(in crate::execution::step) fn step_services(
    &self,
    invocation: InvocationId,
    worktree: &Path,
    environment: &CommandEnvironment,
  ) -> StepServices {
    let (root, run, notifier) = (self.root.clone(), self.run.id(), self.notifier.clone());
    let tools = StepServices {
      services: Arc::clone(&self.services),
      state: self.services.register(invocation),
      root: self.root.clone(),
      project_id: self.project.id(),
      worktree: worktree.to_path_buf(),
      run: self.run.id(),
      vault: Arc::clone(&self.vault),
      inherited: std::env::vars_os().collect(),
      project: environment.clone(),
      crashed: Arc::new(move |crash| {
        let problem = RunnerProblem::ServiceCrashed(crash.clone());
        runner_note::note(&root, (run, invocation), problem, &notifier);
      }),
    };
    self.services.keep_tools(invocation, tools.clone());
    tools
  }
}
