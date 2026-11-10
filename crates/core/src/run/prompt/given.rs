//! Which of a run's pinned instructions and skills reach the agent of a step (`docs/design/team-configuration.md`,
//! "Decided 2026-11-09"): a local run's agents get what the step's profile version chooses, and nothing else. A run
//! made from execution inputs still goes by each body's scope and targets: the execution bundle's profile
//! (`ExecutionProfile`, the team server's contract) carries no such lists, so every team run is one.

use super::ProfileInfo;
use crate::{
  id::ResourceId,
  resource::{Exclusion, Instruction, InstructionTarget, Skill, StepKey},
};

/// How a run decides which of its pinned instructions, skills and tools reach an agent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GivenBy {
  /// What the step's profile version chooses ([`crate::resource::AgentProfile::instruction_resources`],
  /// [`skills`](crate::resource::AgentProfile::skills), [`tools`](crate::resource::AgentProfile::tools)).
  Profile,
  /// The scope and targets of each pinned body, and a tool's callers: a run made from execution inputs, whose
  /// profiles carry no lists.
  Scope,
}

impl<'a> ProfileInfo<'a> {
  /// The agent of this profile at `step` of `pipeline`, as a scope reads it.
  pub fn at<'s>(&'s self, pipeline: ResourceId, step: &'s StepKey) -> InstructionTarget<'s>
  where
    'a: 's,
  {
    InstructionTarget {
      pipeline,
      step,
      profile: self.id,
      role: self.body.role(),
    }
  }

  /// Whether the agent `at` gets the instruction `resource`, whatever the event.
  pub fn gets_instruction(
    &self,
    at: InstructionTarget<'_>,
    resource: ResourceId,
    body: &Instruction,
  ) -> Result<(), Exclusion> {
    match self.given {
      GivenBy::Profile if self.body.instruction_resources().contains(resource) => Ok(()),
      GivenBy::Profile => Err(Exclusion::NotChosen),
      GivenBy::Scope => body.reaches(at),
    }
  }

  /// Whether the agent `at` gets the skill `resource`.
  pub fn gets_skill(&self, at: InstructionTarget<'_>, resource: ResourceId, body: &Skill) -> Result<(), Exclusion> {
    match self.given {
      GivenBy::Profile if self.body.skills().contains(resource) => Ok(()),
      GivenBy::Profile => Err(Exclusion::NotChosen),
      GivenBy::Scope => body.reaches(at),
    }
  }
}
