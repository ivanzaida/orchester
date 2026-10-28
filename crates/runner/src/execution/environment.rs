//! The variables Project settings › Environment adds to every agent CLI and command tool a run
//! starts. They are passed through unchanged, whatever they name. Orchester adds [`RUN_STORAGE`], the run's own
//! storage, unless the project sets it.

use std::collections::BTreeMap;

use orchester_core::{
  connection::stream::RunnerProblem,
  resource::{CommandEnvironment, VariableName},
  store::SettingsStore,
  workspaces::artifacts_folder,
};

use super::run::RunTask;

/// Where the run keeps what its agents write for people to see (ORC-114): `.orchester/runs/<run-id>/`, which the task
/// page lists under Artifacts and cleaning a workspace never touches.
pub(super) const RUN_STORAGE: &str = "ORCHESTER_RUN_STORAGE";

impl RunTask {
  pub(super) fn environment(&self) -> Result<CommandEnvironment, RunnerProblem> {
    let failed = |detail: String| RunnerProblem::CouldNotStart { detail };
    let project = self
      .db
      .project_environment()
      .map_err(|error| failed(error.to_string()))?;
    if project.get(RUN_STORAGE).is_some() {
      return Ok(project);
    }
    let storage = artifacts_folder(&self.root, self.run.id());
    let mut variables: BTreeMap<VariableName, String> = project.into();
    variables.insert(
      VariableName::new(RUN_STORAGE).map_err(|error| failed(error.to_string()))?,
      storage.to_string_lossy().into_owned(),
    );
    CommandEnvironment::new(variables).map_err(|error| failed(error.to_string()))
  }
}
