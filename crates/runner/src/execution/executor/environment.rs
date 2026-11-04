//! The variables Project settings › Environment adds to every agent CLI and command tool a run
//! starts, the merge queue's Check on the merged result included. They are passed through unchanged, whatever they
//! name. Orchester adds [`RUN_STORAGE`], the run's own storage, unless the project sets it.

use std::collections::BTreeMap;

use orchester_core::{
  connection::stream::RunnerProblem,
  id::RunId,
  resource::{CommandEnvironment, VariableName},
  store::SettingsStore,
};

use super::run::RunTask;
use crate::execution_root::ExecutionRoot;

/// Where the run keeps what its agents write for people to see (ORC-114): `.orchester/runs/<run-id>/`, which the task
/// page lists under Artifacts and cleaning a workspace never touches.
pub(super) const RUN_STORAGE: &str = "ORCHESTER_RUN_STORAGE";

impl RunTask {
  pub(super) fn environment(&self) -> Result<CommandEnvironment, RunnerProblem> {
    run_environment(&self.db, &self.execution, self.run.id()).map_err(|detail| RunnerProblem::CouldNotStart { detail })
  }
}

/// The variables the project in `db` adds to what run `run` starts, its storage in `execution`; why they cannot be
/// read when not.
pub(super) fn run_environment(
  db: &impl SettingsStore,
  execution: &ExecutionRoot,
  run: RunId,
) -> Result<CommandEnvironment, String> {
  let project = db.project_environment().map_err(|error| error.to_string())?;
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
