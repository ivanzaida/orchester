//! The checks a step's agent may run with `run_check` (`docs/plan.md`, M12), as the store reads them
//! (`ProjectDb::run_checks`): one whose access rules approve no call runs without asking, one approved once per run
//! asks once, any other asks before each call.

use orchester_core::{connection::stream::RunnerProblem, orchester_tools::ProjectTool, resource::StepKey};

use super::{super::run::RunTask, start::problem};

impl RunTask {
  /// The checks step `step` of the run is offered.
  pub(super) fn offered_checks(&self, step: &StepKey) -> Result<Vec<ProjectTool>, RunnerProblem> {
    self
      .db
      .run_checks(self.run.snapshot(), step)
      .map_err(|error| problem(format!("cannot read the run's checks: {error}")))
  }
}
