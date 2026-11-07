//! When a version *names* a resource, by typed fields of its body and never by a text scan: the ids in a prompt, a
//! description or an instruction's text name nothing. These are the fields `docs/cloud-api.md` lists for the used-by
//! route, read here with core's bodies as the team server reads them. A local resource's version has no `references`
//! list, so only the body counts; ORC-817 moves a team version's references to `TeamResource { kind, server id }`, and
//! what it adds to the rule is added here.

use orchester_core::{
  id::ResourceId,
  resource::{AgentTargets, Instruction, InstructionScope, Pipeline, Skill, Tool},
};

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
