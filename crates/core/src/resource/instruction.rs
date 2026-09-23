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
  /// The agent starts working on a step.
  Start,
  /// The agent continues a step after a pause, interruption or retry.
  Resume,
}

/// Which agents receive an instruction, by agent role (e.g. `Builder`). Every profile with a
/// targeted role receives it, whichever connection or model the profile uses.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentTargets {
  All,
  Roles(BTreeSet<Name>),
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
  #[error("instruction text must not be blank")]
  BlankText,
  #[error("an instruction for selected roles needs at least one role")]
  NoRoles,
  #[error("an instruction needs at least one injection event")]
  NoEvents,
  #[error("an agent role instruction applies to its own role and cannot target other roles")]
  RoleScopeTargetsRoles,
}

/// A project instruction: prompt text injected into selected agents at selected events.
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
    if text.is_blank() {
      return Err(InstructionError::BlankText);
    }
    if events.is_empty() {
      return Err(InstructionError::NoEvents);
    }
    match (&scope, &targets) {
      (_, AgentTargets::Roles(roles)) if roles.is_empty() => return Err(InstructionError::NoRoles),
      (InstructionScope::AgentRole { .. }, AgentTargets::Roles(_)) => {
        return Err(InstructionError::RoleScopeTargetsRoles);
      }
      _ => {}
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
}
