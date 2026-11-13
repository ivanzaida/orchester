//! The checks a step of a run is offered (`run_check`, `docs/plan.md` M12): the command tools with access rules the run
//! pinned that the step's agent gets, each named by its tool's slug and approved as its rules say. A local run's agent
//! gets the tools its pinned profile version chooses (`docs/design/team-configuration.md`, "Decided 2026-11-09"). A
//! run created with execution inputs reads each tool and its slug from them (team authority §7), so a team run needs
//! no tool rows here and a publication after its start offers it nothing new; its profiles carry no lists, so the
//! tools' access rules still say which pipelines and steps call them.

use orchester_core::{
  orchester_tools::ProjectTool,
  resource::{AgentProfile, Slug, StepKey, Tool, ToolAction},
  run::{PinnedVersion, Run, StepExecutor, inputs::RunInputs},
  store::ResourceStore,
};

use super::ProjectDb;
use crate::{StoreError, rows::corrupt};

impl ProjectDb {
  /// The checks step `step` of `run` is offered, in the order the run pinned them.
  pub fn run_checks(&self, run: &Run, step: &StepKey) -> Result<Vec<ProjectTool>, StoreError> {
    let inputs = self.run_inputs(run.id())?;
    let pipeline = run.snapshot().pipeline().resource;
    let chosen = match inputs {
      Some(_) => None,
      None => Some(self.step_profile(run, step)?),
    };
    let mut checks = Vec::new();
    for pinned in run.snapshot().tools() {
      let Some(tool) = self.pinned_tool(inputs.as_ref(), *pinned)? else {
        continue;
      };
      let (ToolAction::Command(action), Some(access)) = (tool.action(), tool.access()) else {
        continue;
      };
      let gets = match &chosen {
        Some(profile) => profile
          .as_ref()
          .is_some_and(|profile| profile.tools().contains(pinned.resource)),
        None => access.allows(pipeline, step),
      };
      if !gets {
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

  /// The profile version step `step` of the local run `run` pinned; `None` for a step no agent runs.
  fn step_profile(&self, run: &Run, step: &StepKey) -> Result<Option<AgentProfile>, StoreError> {
    let Some(StepExecutor::Agent { profile, .. }) = run.snapshot().step(step).map(|pinned| pinned.executor()) else {
      return Ok(None);
    };
    self
      .version::<AgentProfile>(profile.resource, profile.number)?
      .map(|version| Some(version.body().clone()))
      .ok_or_else(|| corrupt("run check", "a profile the run pinned has no such version"))
  }

  /// Tool version `pinned` as the run's inputs hold it, else as it was published; `None` for a team run's tool that
  /// runs no command, which is no check.
  fn pinned_tool(&self, inputs: Option<&RunInputs>, pinned: PinnedVersion) -> Result<Option<Tool>, StoreError> {
    let missing = || corrupt("run check", "a tool the run pinned has no such version");
    match inputs {
      Some(inputs) => Ok(inputs.tool(pinned).ok_or_else(missing)?.command_tool()),
      None => self
        .version::<Tool>(pinned.resource, pinned.number)?
        .map(|version| Some(version.body().clone()))
        .ok_or_else(missing),
    }
  }

  /// The slug of tool `pinned` as the run's inputs label it, else as the tool's resource names it now.
  fn pinned_tool_slug(&self, inputs: Option<&RunInputs>, pinned: PinnedVersion) -> Result<Slug, StoreError> {
    let gone = || corrupt("run check", "a tool the run pinned no longer exists");
    match inputs {
      Some(inputs) => inputs
        .bundle()
        .inputs()
        .label(pinned.resource)
        .map(|label| label.slug().clone())
        .ok_or_else(gone),
      None => self
        .resource(pinned.resource)?
        .map(|resource| resource.slug().clone())
        .ok_or_else(gone),
    }
  }
}
