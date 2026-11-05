//! The variables Project settings › Environment adds to every agent CLI and command tool a run
//! starts, the merge queue's Check on the merged result included. They are passed through unchanged, whatever they
//! name. Orchester adds [`RUN_STORAGE`], the run's own storage, unless the project sets it.
//!
//! The executor never reads them from the project database (`docs/architecture/remote-runners.md`, rule 1): it asks
//! the coordinator link for the project's variables and its computer's resources for what they are on this computer
//! ([`computer_environment`]), at each use, as it read them before.

use std::collections::BTreeMap;

use orchester_core::{
  computer::ComputerError,
  connection::stream::RunnerProblem,
  id::RunId,
  resource::{CommandEnvironment, VariableName},
};
use thiserror::Error;

use super::{
  link::{EnvironmentLink, LinkError, environment::project_environment},
  run::{ExecutionError, RunTask},
};
use crate::{computer::ComputerResources, execution_root::ExecutionRoot};

/// Where the run keeps what its agents write for people to see (ORC-114): `.orchester/runs/<run-id>/`, which the task
/// page lists under Artifacts and cleaning a workspace never touches.
pub(super) const RUN_STORAGE: &str = "ORCHESTER_RUN_STORAGE";

/// Why the environment could not be had: the coordinator did not give the project's variables, or the computer did
/// not make its environment of them.
#[derive(Debug, Error)]
pub(super) enum EnvironmentError {
  #[error(transparent)]
  Link(#[from] LinkError),
  #[error(transparent)]
  Computer(#[from] ComputerError),
}

impl From<EnvironmentError> for ExecutionError {
  fn from(error: EnvironmentError) -> Self {
    match error {
      EnvironmentError::Link(error) => Self::Link(error),
      EnvironmentError::Computer(error) => Self::Computer(error),
    }
  }
}

/// The variables agents and command tools start with on `computer`: the project's, asked through `link`, as the
/// computer gives them.
pub(super) fn computer_environment(
  link: &(impl EnvironmentLink + ?Sized),
  computer: &(impl ComputerResources + ?Sized),
) -> Result<CommandEnvironment, EnvironmentError> {
  let project = project_environment(link)?;
  Ok(computer.environment(project)?)
}

impl RunTask {
  pub(super) fn environment(&self) -> Result<CommandEnvironment, RunnerProblem> {
    run_environment((&*self.link, &*self.computer), &self.execution, self.run.id())
      .map_err(|detail| RunnerProblem::CouldNotStart { detail })
  }

  /// The variables this run's agents and command tools start with on its computer, without the run's storage.
  pub(super) fn computer_environment(&self) -> Result<CommandEnvironment, EnvironmentError> {
    computer_environment(&*self.link, &*self.computer)
  }
}

/// The variables the project adds to what run `run` starts, asked through `link` and given by `computer`, with the
/// run's storage in `execution`; why they cannot be had when not.
pub(super) fn run_environment(
  (link, computer): (&(impl EnvironmentLink + ?Sized), &(impl ComputerResources + ?Sized)),
  execution: &ExecutionRoot,
  run: RunId,
) -> Result<CommandEnvironment, String> {
  let project = computer_environment(link, computer).map_err(|error| error.to_string())?;
  if project.get(RUN_STORAGE).is_some() {
    return Ok(project);
  }
  let storage = execution.run_storage(run);
  let mut variables: BTreeMap<VariableName, String> = project.into();
  variables.insert(
    VariableName::new(RUN_STORAGE).map_err(|error| error.to_string())?,
    storage.to_string_lossy().into_owned(),
  );
  CommandEnvironment::new(variables).map_err(|error| error.to_string())
}
