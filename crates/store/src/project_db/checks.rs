//! The checks a step of a run is offered (`run_check`, `docs/plan.md` M12): the command tools the run pinned whose
//! access rules let the run's pipeline and the step call them, each named by its tool's slug and approved as its
//! rules say. A run created with execution inputs reads each tool and its slug from them (team authority §7), so a
//! team run needs no tool rows here and a publication after its start offers it nothing new.

use orchester_core::{
  orchester_tools::ProjectTool,
  resource::{Slug, StepKey, Tool, ToolAction},
  run::{PinnedVersion, Run, inputs::RunInputs},
  store::ResourceStore,
};

use super::ProjectDb;
use crate::{StoreError, rows::corrupt};

impl ProjectDb {
  /// The checks step `step` of `run` is offered, in the order the run pinned them.
  pub fn run_checks(&self, run: &Run, step: &StepKey) -> Result<Vec<ProjectTool>, StoreError> {
    let inputs = self.run_inputs(run.id())?;
    let pipeline = run.snapshot().pipeline().resource;
    let mut checks = Vec::new();
    for pinned in run.snapshot().tools() {
      let tool = self.pinned_tool(inputs.as_ref(), *pinned)?;
      let (ToolAction::Command(action), Some(access)) = (tool.action(), tool.access()) else {
        continue;
      };
      if !access.allows(pipeline, step) {
        continue;
      }
      let slug = self.pinned_tool_slug(inputs.as_ref(), *pinned)?;
      checks.push(ProjectTool::new(
        slug.as_str(),
        tool.description(),
        action.clone(),
        access.approval(),
      ));
    }
    Ok(checks)
  }

  /// Tool version `pinned` as the run's inputs hold it, else as it was published.
  fn pinned_tool(&self, inputs: Option<&RunInputs>, pinned: PinnedVersion) -> Result<Tool, StoreError> {
    let missing = || corrupt("run check", "a tool the run pinned has no such version");
    match inputs {
      Some(inputs) => inputs.tool(pinned).cloned().ok_or_else(missing),
      None => self
        .version::<Tool>(pinned.resource, pinned.number)?
        .map(|version| version.body().clone())
        .ok_or_else(missing),
    }
  }

  /// The slug of tool `pinned` as the run's inputs label it, else as the tool's resource names it now.
  fn pinned_tool_slug(&self, inputs: Option<&RunInputs>, pinned: PinnedVersion) -> Result<Slug, StoreError> {
    let gone = || corrupt("run check", "a tool the run pinned no longer exists");
    match inputs {
      Some(inputs) => inputs
        .bundle()
        .label(pinned.resource)
        .map(|label| label.slug.clone())
        .ok_or_else(gone),
      None => self
        .resource(pinned.resource)?
        .map(|resource| resource.slug().clone())
        .ok_or_else(gone),
    }
  }
}
