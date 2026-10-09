//! Step inspector fields: the choices a field offers and how a chosen or typed value changes
//! the step. Typed values that cannot be used return the message to show under the field.

use std::num::NonZeroU32;

use orchester_core::{
  id::ResourceId,
  name::Name,
  permission::StepPermissions,
  resource::{FailureRoute, ModelId, PromptText, RepositoryAccess, Step, StepKey, StepKind},
  tokens::{TokenBudget, grouped},
};

use super::problems::prompt_error;
use crate::{
  budget::{read_budget, unbounded},
  messages::{self, Message},
};

/// Retries a new failure route allows until the user changes it.
const DEFAULT_RETRIES: NonZeroU32 = NonZeroU32::MIN;

/// What runs a step, as the profile selector offers it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Executor {
  Unset,
  Profile(ResourceId),
  Tool(ResourceId),
}

impl Executor {
  pub const fn of(step: &Step) -> Self {
    match step.kind {
      StepKind::Agent { profile: Some(id), .. } => Self::Profile(id),
      StepKind::Tool { tool: Some(id) } => Self::Tool(id),
      StepKind::Agent { profile: None, .. } | StepKind::Tool { tool: None } => Self::Unset,
    }
  }

  /// Runs the step with this executor. Choosing a profile for an agent step keeps its
  /// repository access, model and permission narrowing; nothing chosen keeps whether it is an
  /// agent or a tool step.
  pub fn apply(self, step: &mut Step) {
    let (repository, model, permissions) = match &step.kind {
      StepKind::Agent {
        repository,
        model,
        permissions,
        ..
      } => (*repository, model.clone(), permissions.clone()),
      StepKind::Tool { .. } => (RepositoryAccess::ReadOnly, None, StepPermissions::default()),
    };
    step.kind = match (self, &step.kind) {
      (Self::Profile(id), _) => StepKind::Agent {
        profile: Some(id),
        repository,
        model,
        permissions,
      },
      (Self::Tool(id), _) => StepKind::Tool { tool: Some(id) },
      (Self::Unset, StepKind::Agent { .. }) => StepKind::Agent {
        profile: None,
        repository,
        model,
        permissions,
      },
      (Self::Unset, StepKind::Tool { .. }) => StepKind::Tool { tool: None },
    };
  }
}

/// The step's model override, or none to use the profile's model.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelChoice(pub Option<ModelId>);

impl ModelChoice {
  pub fn of(step: &Step) -> Self {
    match &step.kind {
      StepKind::Agent { model, .. } => Self(model.clone()),
      StepKind::Tool { .. } => Self(None),
    }
  }

  pub fn apply(self, step: &mut Step) {
    if let StepKind::Agent { model, .. } = &mut step.kind {
      *model = self.0;
    }
  }
}

/// Where the step returns when changes are requested, or nowhere: the run fails.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RouteChoice(pub Option<StepKey>);

impl RouteChoice {
  pub fn of(step: &Step) -> Self {
    Self(step.on_failure.as_ref().map(|route| route.return_to.clone()))
  }

  /// A new route allows one retry; changing the target keeps the retries.
  pub fn apply(self, step: &mut Step) {
    let retries = step
      .on_failure
      .as_ref()
      .map_or(DEFAULT_RETRIES, |route| route.max_retries);
    step.on_failure = self.0.map(|return_to| FailureRoute {
      return_to,
      max_retries: retries,
    });
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AccessChoice(pub RepositoryAccess);

impl AccessChoice {
  pub const fn of(step: &Step) -> Self {
    match step.kind {
      StepKind::Agent { repository, .. } => Self(repository),
      StepKind::Tool { .. } => Self(RepositoryAccess::ReadOnly),
    }
  }

  pub fn apply(self, step: &mut Step) {
    if let StepKind::Agent { repository, .. } = &mut step.kind {
      *repository = self.0;
    }
  }
}

pub fn parse_name(text: &str) -> Result<Name, Message> {
  Name::new(text).map_err(|error| messages::name_error(&error))
}

pub fn parse_instructions(text: &str) -> Result<PromptText, Message> {
  PromptText::new(text).map_err(|error| prompt_error(&error))
}

/// A step budget field's text, whole tokens grouped with `separator` (the budget input rule);
/// empty text means no step budget.
pub fn parse_budget(text: &str, separator: &str) -> Result<Option<TokenBudget>, Message> {
  if text.trim().is_empty() {
    return Ok(None);
  }
  read_budget(text, separator, unbounded()).map(Some)
}

/// A step budget in its field, grouped for the locale; empty without one.
pub fn budget_text(budget: Option<TokenBudget>, separator: &str) -> String {
  budget
    .map(|budget| grouped(budget.get(), separator))
    .unwrap_or_default()
}

pub fn parse_retries(text: &str) -> Result<NonZeroU32, Message> {
  text
    .trim()
    .parse::<NonZeroU32>()
    .map_err(|_| Message::new("pipelines.problems.invalidRetries"))
}

/// Models a step can override its profile's with: those of the project's published profiles,
/// plus the step's own override if no profile uses it.
pub fn model_choices<'a>(
  profile_models: impl Iterator<Item = &'a ModelId>,
  current: Option<&'a ModelId>,
) -> Vec<ModelId> {
  let mut models: Vec<ModelId> = Vec::new();
  for model in profile_models.chain(current).cloned() {
    if !models.contains(&model) {
      models.push(model);
    }
  }
  models
}

/// A name no step has yet: `first` ("New step"), else `numbered(2)`, `numbered(3)`, … so steps
/// added one after another can be told apart.
pub fn unused_step_name(steps: &[Step], first: &str, numbered: impl Fn(usize) -> String) -> Name {
  let taken = |candidate: &str| steps.iter().any(|step| step.name.as_str() == candidate);
  let name = std::iter::once(first.to_owned())
    .chain((2..).map(numbered))
    .find(|candidate| !taken(candidate))
    .expect("a free name exists among more candidates than there are steps");
  Name::new(&name).expect("new step names from the locale files are not blank")
}
