//! Where each part of a prompt came from: the prompt's parts in assembly order and the source of
//! each section, as the runner stores them with an invocation and a preview shows them.

use serde::{Deserialize, Serialize};

use crate::{
  id::{ResourceId, TaskId},
  resource::{InjectionEvent, InstructionScope, StepKey, VersionNumber},
};

/// The parts of a prompt, in the order they are assembled (`docs/design/orchester-workspace.md`,
/// "Instructions"): agent role → project rules → pipeline instructions → step instructions → task
/// context.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PromptPart {
  AgentRole,
  ProjectRules,
  Pipeline,
  Step,
  Task,
}

impl PromptPart {
  /// The part an instruction of `scope` belongs to.
  pub const fn of(scope: &InstructionScope) -> Self {
    match scope {
      InstructionScope::Project => Self::ProjectRules,
      InstructionScope::AgentRole { .. } => Self::AgentRole,
      InstructionScope::Pipeline { .. } => Self::Pipeline,
      InstructionScope::Step { .. } => Self::Step,
    }
  }
}

/// The version of an instruction a prompt used.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "state")]
pub enum InstructionVersion {
  Published {
    number: VersionNumber,
  },
  /// A draft substituted in a preview. Agents only ever receive published versions.
  Draft {
    based_on: Option<VersionNumber>,
  },
}

/// The source of one section of a prompt.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "source")]
pub enum PromptSource {
  /// The agent profile's responsibilities and role instructions.
  AgentProfile {
    resource: ResourceId,
    version: VersionNumber,
  },
  /// An instruction resource of the project.
  Instruction {
    resource: ResourceId,
    part: PromptPart,
    version: InstructionVersion,
  },
  /// The step's own instructions, from the pipeline version.
  Step {
    pipeline: ResourceId,
    version: VersionNumber,
    step: StepKey,
  },
  /// The task's brief and criteria, the outputs of earlier steps and how to report the result.
  Task { task: TaskId },
}

impl PromptSource {
  pub const fn part(&self) -> PromptPart {
    match self {
      Self::AgentProfile { .. } => PromptPart::AgentRole,
      Self::Instruction { part, .. } => *part,
      Self::Step { .. } => PromptPart::Step,
      Self::Task { .. } => PromptPart::Task,
    }
  }
}

/// What an agent CLI launch was told and where each part came from, stored with the invocation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InvocationPrompt {
  pub event: InjectionEvent,
  /// In the order the prompt holds them.
  pub sources: Vec<PromptSource>,
}
