use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::{pipeline::StepKey, prompt::PromptText};
use crate::{id::ResourceId, name::Name};

/// Point in an agent's life at which an instruction is injected. The design leaves the final
/// vocabulary open; new events are added as variants, and stored names stay stable.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InjectionEvent {
  /// The agent starts working on a step: a new session.
  Start,
  /// The agent continues a step after a pause, interruption or limit: its session resumes.
  Resume,
}

impl InjectionEvent {
  pub const ALL: [Self; 2] = [Self::Start, Self::Resume];
}

/// Which agents receive an instruction: every agent, the profiles with one of the roles (e.g.
/// `Builder`, whichever connection or model they use), or the named profiles only.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentTargets {
  All,
  Roles(BTreeSet<Name>),
  Profiles(BTreeSet<ResourceId>),
}

/// Where an instruction sits in prompt assembly: agent role → project rules → pipeline
/// instructions → step instructions → task context.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstructionScope {
  Project,
  AgentRole { role: Name },
  Pipeline { pipeline: ResourceId },
  Step { pipeline: ResourceId, step: StepKey },
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum InstructionError {
  #[error("an instruction for selected agents needs at least one role or profile")]
  NoTargets,
  #[error("an instruction needs at least one injection event")]
  NoEvents,
  #[error("an agent role instruction applies to its own role and cannot select other agents")]
  RoleScopeSelectsAgents,
}

/// The agent an instruction could be injected into: which pipeline step it works on and which
/// profile runs it.
#[derive(Clone, Copy, Debug)]
pub struct InstructionTarget<'a> {
  pub pipeline: ResourceId,
  pub step: &'a StepKey,
  pub profile: ResourceId,
  pub role: &'a Name,
}

/// Why an instruction does not reach an agent at an event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Exclusion {
  /// An agent role instruction for another role.
  OtherRole,
  /// It selects other roles or profiles.
  OtherAgents,
  /// A pipeline or step instruction of another pipeline.
  OtherPipeline,
  /// A step instruction of another step of the same pipeline.
  OtherStep,
  /// It is injected at other events.
  OtherEvent,
}

/// A project instruction: prompt text injected into selected agents at selected events. A draft
/// may have blank text; publishing requires some ([`crate::resource::ResourceBody`]).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "InstructionFields")]
pub struct Instruction {
  text: PromptText,
  targets: AgentTargets,
  events: BTreeSet<InjectionEvent>,
  scope: InstructionScope,
}

#[derive(Deserialize)]
struct InstructionFields {
  text: PromptText,
  targets: AgentTargets,
  events: BTreeSet<InjectionEvent>,
  scope: InstructionScope,
}

impl TryFrom<InstructionFields> for Instruction {
  type Error = InstructionError;

  fn try_from(fields: InstructionFields) -> Result<Self, InstructionError> {
    Self::new(fields.text, fields.targets, fields.events, fields.scope)
  }
}

impl Instruction {
  pub fn new(
    text: PromptText,
    targets: AgentTargets,
    events: BTreeSet<InjectionEvent>,
    scope: InstructionScope,
  ) -> Result<Self, InstructionError> {
    if events.is_empty() {
      return Err(InstructionError::NoEvents);
    }
    let selects_none = match &targets {
      AgentTargets::All => false,
      AgentTargets::Roles(roles) => roles.is_empty(),
      AgentTargets::Profiles(profiles) => profiles.is_empty(),
    };
    if selects_none {
      return Err(InstructionError::NoTargets);
    }
    if matches!(scope, InstructionScope::AgentRole { .. }) && targets != AgentTargets::All {
      return Err(InstructionError::RoleScopeSelectsAgents);
    }
    Ok(Self {
      text,
      targets,
      events,
      scope,
    })
  }

  pub fn text(&self) -> &PromptText {
    &self.text
  }

  pub fn targets(&self) -> &AgentTargets {
    &self.targets
  }

  pub fn events(&self) -> &BTreeSet<InjectionEvent> {
    &self.events
  }

  pub fn scope(&self) -> &InstructionScope {
    &self.scope
  }

  /// Whether the instruction is for `target`'s agent, whatever the event.
  pub fn reaches(&self, target: InstructionTarget<'_>) -> Result<(), Exclusion> {
    match &self.scope {
      InstructionScope::Project => {}
      InstructionScope::AgentRole { role } if role == target.role => {}
      InstructionScope::AgentRole { .. } => return Err(Exclusion::OtherRole),
      InstructionScope::Pipeline { pipeline } if *pipeline == target.pipeline => {}
      InstructionScope::Step { pipeline, step } if *pipeline == target.pipeline => {
        if step != target.step {
          return Err(Exclusion::OtherStep);
        }
      }
      InstructionScope::Pipeline { .. } | InstructionScope::Step { .. } => return Err(Exclusion::OtherPipeline),
    }
    let selected = match &self.targets {
      AgentTargets::All => true,
      AgentTargets::Roles(roles) => roles.contains(target.role),
      AgentTargets::Profiles(profiles) => profiles.contains(&target.profile),
    };
    if selected { Ok(()) } else { Err(Exclusion::OtherAgents) }
  }

  /// Whether the instruction is injected into `target`'s agent at `event`.
  pub fn applies(&self, target: InstructionTarget<'_>, event: InjectionEvent) -> Result<(), Exclusion> {
    self.reaches(target)?;
    if self.events.contains(&event) {
      Ok(())
    } else {
      Err(Exclusion::OtherEvent)
    }
  }
}
