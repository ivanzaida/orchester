use std::{
  collections::{BTreeSet, HashMap},
  fmt,
  num::NonZeroU32,
};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::{
  agent_profile::ModelId,
  lifecycle::{Slug, SlugError},
  prompt::PromptText,
};
use crate::{id::ResourceId, money::Money, name::Name};

/// Stable key of a step within its pipeline, e.g. `implement`. Kept across versions so
/// instructions, tool access rules and failure routes can refer to a step.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct StepKey(Slug);

impl StepKey {
  pub fn new(value: &str) -> Result<Self, SlugError> {
    Slug::new(value).map(Self)
  }

  pub fn as_str(&self) -> &str {
    self.0.as_str()
  }
}

impl fmt::Display for StepKey {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(self.as_str())
  }
}

impl TryFrom<String> for StepKey {
  type Error = SlugError;

  fn try_from(value: String) -> Result<Self, SlugError> {
    Self::new(&value)
  }
}

impl From<StepKey> for String {
  fn from(key: StepKey) -> Self {
    key.0.as_str().to_owned()
  }
}

/// What an agent step may do to the run's working tree.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RepositoryAccess {
  ReadOnly,
  Write,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepKind {
  /// Runs an agent of the selected profile, with the profile's model unless `model`
  /// overrides it for this step.
  Agent {
    profile: ResourceId,
    repository: RepositoryAccess,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    model: Option<ModelId>,
  },
  /// Calls a project tool under the tool's access policy.
  Tool { tool: ResourceId },
}

/// When a step fails, the run returns to `return_to` (an upstream step) at most `max_retries`
/// times before the run fails.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FailureRoute {
  pub return_to: StepKey,
  pub max_retries: NonZeroU32,
}

/// One pipeline step. Rules that span steps (unique keys, known dependencies, no cycles,
/// upstream retry targets) are enforced by [`Pipeline::new`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Step {
  pub key: StepKey,
  pub name: Name,
  pub kind: StepKind,
  pub instructions: PromptText,
  /// Steps that must pass before this one starts; steps with the same satisfied
  /// dependencies run in parallel.
  pub depends_on: BTreeSet<StepKey>,
  /// A person approves the step's result before dependent steps start, e.g. plan approval.
  pub approval_gate: bool,
  pub on_failure: Option<FailureRoute>,
  pub budget: Option<Money>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PipelineError {
  #[error("a pipeline needs at least one step")]
  NoSteps,
  #[error("step key `{0}` is used more than once")]
  DuplicateStep(StepKey),
  #[error("step `{step}` depends on `{dependency}`, which is not a step of this pipeline")]
  UnknownDependency { step: StepKey, dependency: StepKey },
  #[error("step `{0}` depends on itself through its dependencies")]
  Cycle(StepKey),
  #[error("step `{step}` returns to `{target}` on failure, which is not a step of this pipeline")]
  UnknownRetryTarget { step: StepKey, target: StepKey },
  #[error("step `{step}` returns to `{target}` on failure, but `{target}` does not run before it")]
  RetryTargetNotUpstream { step: StepKey, target: StepKey },
  #[error("step `{0}` has a budget that is not greater than zero")]
  NonPositiveBudget(StepKey),
}

/// Ordered steps forming an acyclic dependency graph.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "PipelineFields")]
pub struct Pipeline {
  steps: Vec<Step>,
}

#[derive(Deserialize)]
struct PipelineFields {
  steps: Vec<Step>,
}

impl TryFrom<PipelineFields> for Pipeline {
  type Error = PipelineError;

  fn try_from(fields: PipelineFields) -> Result<Self, PipelineError> {
    Self::new(fields.steps)
  }
}

impl Pipeline {
  pub fn new(steps: Vec<Step>) -> Result<Self, PipelineError> {
    if steps.is_empty() {
      return Err(PipelineError::NoSteps);
    }
    let graph = Graph::new(&steps)?;
    for step in &steps {
      check_budget(step)?;
      graph.check_route(step)?;
    }
    Ok(Self { steps })
  }

  /// Steps in the order the user arranged them.
  pub fn steps(&self) -> &[Step] {
    &self.steps
  }

  pub fn step(&self, key: &StepKey) -> Option<&Step> {
    self.steps.iter().find(|step| &step.key == key)
  }
}

fn check_budget(step: &Step) -> Result<(), PipelineError> {
  match step.budget {
    Some(budget) if budget.micros() <= 0 => Err(PipelineError::NonPositiveBudget(step.key.clone())),
    _ => Ok(()),
  }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Visit {
  InProgress,
  Done,
}

/// Dependency graph over step positions, validated on construction.
struct Graph<'a> {
  steps: &'a [Step],
  index: HashMap<&'a StepKey, usize>,
}

impl<'a> Graph<'a> {
  fn new(steps: &'a [Step]) -> Result<Self, PipelineError> {
    let mut index = HashMap::with_capacity(steps.len());
    for (position, step) in steps.iter().enumerate() {
      if index.insert(&step.key, position).is_some() {
        return Err(PipelineError::DuplicateStep(step.key.clone()));
      }
    }
    for step in steps {
      if let Some(missing) = step.depends_on.iter().find(|key| !index.contains_key(key)) {
        return Err(PipelineError::UnknownDependency {
          step: step.key.clone(),
          dependency: missing.clone(),
        });
      }
    }
    let graph = Self { steps, index };
    graph.check_acyclic()?;
    Ok(graph)
  }

  fn dependencies(&self, position: usize) -> impl Iterator<Item = usize> + '_ {
    self.steps[position].depends_on.iter().map(|key| self.index[key])
  }

  fn check_acyclic(&self) -> Result<(), PipelineError> {
    let mut visits = vec![None; self.steps.len()];
    for position in 0..self.steps.len() {
      self.visit(position, &mut visits)?;
    }
    Ok(())
  }

  fn visit(&self, position: usize, visits: &mut [Option<Visit>]) -> Result<(), PipelineError> {
    match visits[position] {
      Some(Visit::Done) => return Ok(()),
      Some(Visit::InProgress) => return Err(PipelineError::Cycle(self.steps[position].key.clone())),
      None => {}
    }
    visits[position] = Some(Visit::InProgress);
    for dependency in self.dependencies(position) {
      self.visit(dependency, visits)?;
    }
    visits[position] = Some(Visit::Done);
    Ok(())
  }

  fn check_route(&self, step: &Step) -> Result<(), PipelineError> {
    let Some(route) = &step.on_failure else {
      return Ok(());
    };
    let Some(&target) = self.index.get(&route.return_to) else {
      return Err(PipelineError::UnknownRetryTarget {
        step: step.key.clone(),
        target: route.return_to.clone(),
      });
    };
    if self.is_upstream(target, self.index[&step.key]) {
      Ok(())
    } else {
      Err(PipelineError::RetryTargetNotUpstream {
        step: step.key.clone(),
        target: route.return_to.clone(),
      })
    }
  }

  /// Whether `target` is a direct or transitive dependency of `position`.
  fn is_upstream(&self, target: usize, position: usize) -> bool {
    let mut pending: Vec<usize> = self.dependencies(position).collect();
    let mut seen = vec![false; self.steps.len()];
    while let Some(current) = pending.pop() {
      if current == target {
        return true;
      }
      if !std::mem::replace(&mut seen[current], true) {
        pending.extend(self.dependencies(current));
      }
    }
    false
  }
}
