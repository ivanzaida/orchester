//! When a version *names* a resource, by typed data and never by a text scan: the ids in a prompt, a description or an
//! instruction's text name nothing. A local resource's version is read by the fields of its body, with core's bodies,
//! and a local project's own rule decides which: a profile names what it gives its agents
//! ([`profile`]), where the team server reads an instruction's or skill's scope and targets and a tool's callers.

use orchester_core::{
  id::ResourceId,
  resource::{AgentProfile, Pipeline},
};

/// A kind whose body names no resource here: a team MCP server or reviewer names what it uses in its references, and a
/// local instruction, skill or tool names nothing ([`profile`]).
pub(super) fn never<B>(_: &B, _: ResourceId) -> bool {
  false
}

/// A pipeline names its steps' executors, an agent profile or a tool, and the profile its Deliver step integrates
/// with.
pub(super) fn pipeline(pipeline: &Pipeline, target: ResourceId) -> bool {
  pipeline.steps_run_by(target).next().is_some()
    || pipeline.deliver().and_then(|deliver| deliver.integrator()) == Some(target)
}

/// A local profile names the instructions, skills and tools it gives its agents (`docs/design/team-configuration.md`,
/// "Decided 2026-11-09"). An instruction's scope and targets, a skill's scope and a tool's callers name nothing in a
/// local project: they decide nothing for its runs.
pub(super) fn profile(profile: &AgentProfile, target: ResourceId) -> bool {
  profile.gives(target)
}
