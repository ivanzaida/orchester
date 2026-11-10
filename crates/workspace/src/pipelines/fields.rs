//! Step inspector fields: the choices a field offers and how a chosen or typed value changes
//! the step. Typed values that cannot be used return the message to show under the field.

use std::num::NonZeroU32;

use orchester_core::{
  connection::AgentProvider,
  id::ResourceId,
  name::Name,
  permission::StepPermissions,
  resource::{FailureRoute, ModelId, PromptText, RepositoryAccess, Step, StepKey, StepKind},
  tokens::{TokenBudget, grouped},
};

use super::problems::prompt_error;
use crate::{
  budget::read_optional_budget,
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

  /// [`Self::apply`], keeping the step's model override only while the step runs on the CLI it was set for: a model
  /// one CLI knows means nothing to another. `from` is the CLI of the profile that ran the step and `to` that of the
  /// one that runs it now, each `None` when it is not known (no profile, or one not published here): an override is
  /// not dropped on a guess. A tool runs no model, so the override goes with a tool too. Returns the override dropped.
  pub fn apply_on(
    self,
    step: &mut Step,
    (from, to): (Option<AgentProvider>, Option<AgentProvider>),
  ) -> Option<ModelId> {
    let before = ModelChoice::of(step).0;
    self.apply(step);
    let elsewhere = matches!(self, Self::Tool(_)) || matches!((from, to), (Some(from), Some(to)) if from != to);
    let dropped = before.filter(|_| elsewhere)?;
    ModelChoice(None).apply(step);
    Some(dropped)
  }
}

/// The model override the step had when the edit began (`started`) and `step` no longer has, now that it runs something
/// else (a profile on another CLI or a tool, [`Executor::apply_on`]); `None` when it kept it, never had one, or still
/// runs what it started with.
pub fn dropped_override(started: &Step, step: &Step) -> Option<ModelId> {
  if Executor::of(step) == Executor::of(started) || ModelChoice::of(step).0.is_some() {
    return None;
  }
  ModelChoice::of(started).0
}

/// Puts the override the step started with (`started`) back on `step` when `step` runs on the CLI it was set for again
/// (`runs_on` is `was_for`, both known) and has none of its own: choosing another CLI dropped it, choosing its CLI
/// again takes it back, so a stored override is not lost by a choice taken back. Returns whether it did.
pub fn take_back_override(
  step: &mut Step,
  started: &Step,
  (was_for, runs_on): (Option<AgentProvider>, Option<AgentProvider>),
) -> bool {
  let own_cli = was_for.is_some() && was_for == runs_on;
  let Some(model) = ModelChoice::of(started).0.filter(|_| own_cli) else {
    return false;
  };
  if !matches!(step.kind, StepKind::Agent { model: None, .. }) {
    return false;
  }
  ModelChoice(Some(model)).apply(step);
  true
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
  read_optional_budget(text, separator)
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

/// A name no step has yet: `first` (the app's "New step 4", by the number the step adds), else `numbered(2)`,
/// `numbered(3)`, … so steps added one after another can be told apart.
pub fn unused_step_name(steps: &[Step], first: &str, numbered: impl Fn(usize) -> String) -> Name {
  let taken = |candidate: &str| steps.iter().any(|step| step.name.as_str() == candidate);
  let name = std::iter::once(first.to_owned())
    .chain((2..).map(numbered))
    .find(|candidate| !taken(candidate))
    .expect("a free name exists among more candidates than there are steps");
  Name::new(&name).expect("new step names from the locale files are not blank")
}
