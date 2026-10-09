//! The checks a step's agent may run with `run_check` (`docs/plan.md`, M12): the command tools of the project's Tools
//! library that the run pinned and whose access rules let the run's pipeline and this step call them. One whose rules
//! approve no call (`ApprovalMode::Never`) runs without asking; any other asks a person before each call. A check is
//! named by its tool's slug.

use orchester_core::{
  connection::stream::RunnerProblem,
  orchester_tools::ProjectTool,
  resource::{ApprovalMode, StepKey, Tool, ToolAction},
  store::ResourceStore,
};

use super::{super::run::RunTask, start::problem};

impl RunTask {
  /// The checks step `step` of the run is offered.
  pub(super) fn offered_checks(&self, step: &StepKey) -> Result<Vec<ProjectTool>, RunnerProblem> {
    let pipeline = self.run.snapshot().pipeline().resource;
    let unreadable = |error: &dyn std::error::Error| problem(format!("cannot read the run's checks: {error}"));
    let mut checks = Vec::new();
    for pinned in self.run.snapshot().tools() {
      let version = self
        .db
        .version::<Tool>(pinned.resource, pinned.number)
        .map_err(|error| unreadable(&error))?
        .ok_or_else(|| problem("a tool the run pinned no longer exists".to_owned()))?;
      let (ToolAction::Command(action), Some(access)) = (version.body().action(), version.body().access()) else {
        continue;
      };
      if !access.allows(pipeline, step) {
        continue;
      }
      let resource = self
        .db
        .resource(pinned.resource)
        .map_err(|error| unreadable(&error))?
        .ok_or_else(|| problem("a tool the run pinned no longer exists".to_owned()))?;
      checks.push(ProjectTool::new(
        resource.slug().as_str(),
        version.body().description(),
        action.clone(),
        access.approval() != ApprovalMode::Never,
      ));
    }
    Ok(checks)
  }
}
