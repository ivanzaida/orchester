//! The variables Project settings › Environment adds to every agent CLI and command tool a run
//! starts. They are passed through unchanged, whatever they name.

use orchester_core::{connection::stream::RunnerProblem, resource::CommandEnvironment, store::SettingsStore};

use super::run::RunTask;

impl RunTask {
  pub(super) fn environment(&self) -> Result<CommandEnvironment, RunnerProblem> {
    self
      .db
      .project_environment()
      .map_err(|error| RunnerProblem::CouldNotStart {
        detail: error.to_string(),
      })
  }
}
