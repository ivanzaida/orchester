//! The checks a step of a run is offered (`run_check`, `docs/plan.md` M12): the command tools the run pinned whose
//! access rules let the run's pipeline and the step call them, each named by its tool's slug and approved as its
//! rules say.

use orchester_core::{
  orchester_tools::ProjectTool,
  resource::{StepKey, Tool, ToolAction},
  run::RunSnapshot,
  store::ResourceStore,
};

use super::ProjectDb;
use crate::{StoreError, rows::corrupt};

impl ProjectDb {
  /// The checks step `step` of the run pinned as `snapshot` is offered, in the order the run pinned them.
  pub fn run_checks(&self, snapshot: &RunSnapshot, step: &StepKey) -> Result<Vec<ProjectTool>, StoreError> {
    let pipeline = snapshot.pipeline().resource;
    let mut checks = Vec::new();
    for pinned in snapshot.tools() {
      let version = self
        .version::<Tool>(pinned.resource, pinned.number)?
        .ok_or_else(|| corrupt("run check", "a tool the run pinned has no such version"))?;
      let (ToolAction::Command(action), Some(access)) = (version.body().action(), version.body().access()) else {
        continue;
      };
      if !access.allows(pipeline, step) {
        continue;
      }
      let resource = self
        .resource(pinned.resource)?
        .ok_or_else(|| corrupt("run check", "a tool the run pinned no longer exists"))?;
      checks.push(ProjectTool::new(
        resource.slug().as_str(),
        version.body().description(),
        action.clone(),
        access.approval(),
      ));
    }
    Ok(checks)
  }
}
