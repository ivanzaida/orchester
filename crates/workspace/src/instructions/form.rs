//! The instruction editor's values as chosen (`jNTVN` editor in `yRKeF`): who it applies to, when
//! it is injected, where it applies and its prompt, and the mapping to an `Instruction`.

use std::collections::BTreeSet;

use orchester_core::{
  id::ResourceId,
  name::Name,
  resource::{AgentTargets, InjectionEvent, Instruction, InstructionError, InstructionScope, PromptText, StepKey},
};

use crate::{agents::form::prompt_error, messages::Message};

/// A value picked in a select. A newtype so domain values can be held in signals.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Picked<T>(pub T);

/// One choice of "Apply to": every agent, the profiles with a role, or one profile.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Target {
  All,
  Role(Name),
  Profile(ResourceId),
}

impl Target {
  /// The choices that stand for `targets`.
  pub fn of(targets: &AgentTargets) -> Vec<Self> {
    match targets {
      AgentTargets::All => vec![Self::All],
      AgentTargets::Roles(roles) => roles.iter().cloned().map(Self::Role).collect(),
      AgentTargets::Profiles(profiles) => profiles.iter().copied().map(Self::Profile).collect(),
    }
  }

  /// The choices after one was added or removed: all agents excludes every other choice, and roles
  /// and profiles are not mixed, so the newest choice decides which kind stays.
  pub fn after_change(previous: &[Self], next: Vec<Self>) -> Vec<Self> {
    let added = next.iter().find(|choice| !previous.contains(choice)).cloned();
    match added {
      Some(Self::All) => vec![Self::All],
      Some(Self::Role(_)) => next
        .into_iter()
        .filter(|choice| matches!(choice, Self::Role(_)))
        .collect(),
      Some(Self::Profile(_)) => next
        .into_iter()
        .filter(|choice| matches!(choice, Self::Profile(_)))
        .collect(),
      None => next,
    }
  }
}

/// "Inject at" (`UVlYr`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Events {
  Start,
  Resume,
  StartAndResume,
}

impl Events {
  pub const ALL: [Self; 3] = [Self::Start, Self::Resume, Self::StartAndResume];

  pub fn of(events: &BTreeSet<InjectionEvent>) -> Self {
    match (
      events.contains(&InjectionEvent::Start),
      events.contains(&InjectionEvent::Resume),
    ) {
      (true, true) => Self::StartAndResume,
      (false, true) => Self::Resume,
      _ => Self::Start,
    }
  }

  /// The events chosen.
  pub fn set(self) -> BTreeSet<InjectionEvent> {
    match self {
      Self::Start => BTreeSet::from([InjectionEvent::Start]),
      Self::Resume => BTreeSet::from([InjectionEvent::Resume]),
      Self::StartAndResume => BTreeSet::from(InjectionEvent::ALL),
    }
  }
}

/// Where the instruction applies, as its scope tab chose it; the role, pipeline or step may be
/// missing while nothing is chosen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ScopeChoice {
  Project,
  AgentRole(Option<Name>),
  Pipeline(Option<ResourceId>),
  Step(Option<ResourceId>, Option<StepKey>),
}

impl ScopeChoice {
  pub fn of(scope: &InstructionScope) -> Self {
    match scope {
      InstructionScope::Project => Self::Project,
      InstructionScope::AgentRole { role } => Self::AgentRole(Some(role.clone())),
      InstructionScope::Pipeline { pipeline } => Self::Pipeline(Some(*pipeline)),
      InstructionScope::Step { pipeline, step } => Self::Step(Some(*pipeline), Some(step.clone())),
    }
  }

  fn scope(&self) -> Result<InstructionScope, SettingsError> {
    Ok(match self {
      Self::Project => InstructionScope::Project,
      Self::AgentRole(role) => InstructionScope::AgentRole {
        role: role.clone().ok_or(SettingsError::RoleRequired)?,
      },
      Self::Pipeline(pipeline) => InstructionScope::Pipeline {
        pipeline: pipeline.ok_or(SettingsError::PipelineRequired)?,
      },
      Self::Step(pipeline, step) => InstructionScope::Step {
        pipeline: pipeline.ok_or(SettingsError::PipelineRequired)?,
        step: step.clone().ok_or(SettingsError::StepRequired)?,
      },
    })
  }
}

/// The editor's values, as entered.
pub struct InstructionForm<'a> {
  pub text: &'a str,
  pub targets: &'a [Target],
  pub events: Events,
  pub scope: &'a ScopeChoice,
}

/// Why the instruction cannot be saved yet: its prompt, or the other settings.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FormErrors {
  pub text: Option<Message>,
  pub settings: Option<SettingsError>,
}
impl FormErrors {
  /// Whether the Apply to field needs a selection.
  pub fn targets_invalid(&self) -> bool {
    self.settings == Some(SettingsError::TargetsRequired)
  }
}

/// What the settings other than the prompt are missing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettingsError {
  RoleRequired,
  PipelineRequired,
  StepRequired,
  TargetsRequired,
  /// A combination the editor does not offer.
  Invalid,
}

impl SettingsError {
  pub const fn message(self) -> Message {
    Message::new(match self {
      Self::RoleRequired => "instructions.errors.roleRequired",
      Self::PipelineRequired => "instructions.errors.pipelineRequired",
      Self::StepRequired => "instructions.errors.stepRequired",
      Self::TargetsRequired => "instructions.errors.targetsRequired",
      Self::Invalid => "instructions.errors.settingsInvalid",
    })
  }
}

impl InstructionForm<'_> {
  pub fn instruction(&self) -> Result<Instruction, FormErrors> {
    let text = PromptText::new(self.text).map_err(|error| prompt_error(&error));
    let scope = self.scope.scope();
    let (text, scope) = match (text, scope) {
      (Ok(text), Ok(scope)) => (text, scope),
      (text, scope) => {
        return Err(FormErrors {
          text: text.err(),
          settings: scope.err(),
        });
      }
    };
    // An agent role instruction applies to every agent of its role.
    let targets = if matches!(scope, InstructionScope::AgentRole { .. }) {
      AgentTargets::All
    } else {
      self.targets()
    };
    Instruction::new(text, targets, self.events.set(), scope).map_err(|error| FormErrors {
      text: None,
      settings: Some(settings_error(&error)),
    })
  }

  fn targets(&self) -> AgentTargets {
    if self.targets.contains(&Target::All) {
      return AgentTargets::All;
    }
    let roles: BTreeSet<Name> = self
      .targets
      .iter()
      .filter_map(|target| match target {
        Target::Role(role) => Some(role.clone()),
        Target::All | Target::Profile(_) => None,
      })
      .collect();
    if !roles.is_empty() {
      return AgentTargets::Roles(roles);
    }
    AgentTargets::Profiles(
      self
        .targets
        .iter()
        .filter_map(|target| match target {
          Target::Profile(id) => Some(*id),
          Target::All | Target::Role(_) => None,
        })
        .collect(),
    )
  }
}

const fn settings_error(error: &InstructionError) -> SettingsError {
  match error {
    InstructionError::NoTargets => SettingsError::TargetsRequired,
    // The editor offers events only in combinations that have at least one, and gives an agent
    // role instruction every agent of its role.
    InstructionError::NoEvents | InstructionError::RoleScopeSelectsAgents => SettingsError::Invalid,
  }
}
