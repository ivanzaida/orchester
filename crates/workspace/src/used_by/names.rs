//! When a version *names* a resource, by typed data and never by a text scan: the ids in a prompt, a description or an
//! instruction's text name nothing. These are the rules `docs/cloud-api.md` lists for the used-by route, read here as
//! the team server reads them: the `TeamResource` references of a cached team version ([`referenced`], of any kind),
//! and the fields of the body, read with core's bodies. A local resource's version has no `references` list, so only
//! its body counts.

use orchester_core::{
  id::ResourceId,
  resource::{AgentTargets, Instruction, InstructionScope, Pipeline, Skill, Tool},
};
use orchester_store::{ReferenceTarget, VersionReference};

/// A kind whose body names no resource: a team profile, MCP server or reviewer names what it uses in its references.
pub(super) fn never<B>(_: &B, _: ResourceId) -> bool {
  false
}

/// Whether one of `references` is a team resource with `target`'s server id. The reference's kind is not compared with
/// the target's: a reference of the wrong kind is refused when published, not read here.
pub(super) fn referenced(references: &[VersionReference], target: ResourceId) -> bool {
  references
    .iter()
    .any(|reference| matches!(reference.target, ReferenceTarget::TeamResource { id, .. } if id == target))
}

/// A pipeline names its steps' executors, an agent profile or a tool, and the profile its Deliver step integrates
/// with.
pub(super) fn pipeline(pipeline: &Pipeline, target: ResourceId) -> bool {
  pipeline.steps_run_by(target).next().is_some()
    || pipeline.deliver().and_then(|deliver| deliver.integrator()) == Some(target)
}

/// An instruction names the profiles it targets and the pipeline it is scoped to.
pub(super) fn instruction(instruction: &Instruction, target: ResourceId) -> bool {
  matches!(instruction.targets(), AgentTargets::Profiles(profiles) if profiles.contains(&target))
    || scope(instruction.scope(), target)
}

/// A skill names the pipeline it is scoped to.
pub(super) fn skill(skill: &Skill, target: ResourceId) -> bool {
  scope(skill.scope(), target)
}

/// A tool names the pipelines its access policy lists.
pub(super) fn tool(tool: &Tool, target: ResourceId) -> bool {
  tool
    .access()
    .is_some_and(|access| access.callers().contains_key(&target))
}

fn scope(scope: &InstructionScope, target: ResourceId) -> bool {
  match scope {
    InstructionScope::Pipeline { pipeline } | InstructionScope::Step { pipeline, .. } => *pipeline == target,
    InstructionScope::Project | InstructionScope::AgentRole { .. } => false,
  }
}
