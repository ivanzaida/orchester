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
use crate::{id::ResourceId, name::Name, permission::StepPermissions, tokens::TokenBudget};

mod base_moves;
mod budget;
mod changes;
mod deliver;
mod delivers;
mod edit;
mod sources;
mod starter;
mod strictness;

pub use self::{
  base_moves::{takes_base_moves, takes_base_moves_by_default},
  changes::{DeliverChange, PipelineChange, StepChange, changes},
  deliver::{DeliverMode, DeliverStep, MergeMethod},
  delivers::{Deliverable, UnknownDeliverable, WithoutCommits},
  edit::{EditProblem, PipelineEdit},
  sources::{PipelineSources, PipelineSourcesError},
  starter::{StarterAssignments, StarterStep, StarterStepText, feature_delivery},
  strictness::{Weaker, weaker_than},
};

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

/// What an agent may do to the run's working tree: read it, or edit files inside it. A profile's
/// permissions hold one; a step's own value can only narrow it
/// ([`crate::permission::AgentPermissions::narrowed`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RepositoryAccess {
  ReadOnly,
  Write,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepKind {
  /// Runs an agent of the selected profile, with the profile's model unless `model`
  /// overrides it for this step, and the profile's permissions narrowed by `repository` and
  /// `permissions`. A draft may leave the profile unset (`None`); publishing requires it, see
  /// [`Pipeline::unassigned_steps`].
  Agent {
    profile: Option<ResourceId>,
    repository: RepositoryAccess,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    model: Option<ModelId>,
    /// Pipelines saved before steps could narrow commands and network have none.
    #[serde(default, skip_serializing_if = "StepPermissions::is_as_profile")]
    permissions: StepPermissions,
  },
  /// Calls a project tool under the tool's access policy. Unset (`None`) like an agent step's
  /// profile until a tool is chosen.
  Tool { tool: Option<ResourceId> },
}

impl StepKind {
  /// The agent profile or tool that executes the step, once chosen.
  pub const fn executor(&self) -> Option<ResourceId> {
    match self {
      Self::Agent { profile, .. } => *profile,
      Self::Tool { tool } => *tool,
    }
  }
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
  /// Most tokens one execution of the step may use; reaching it stops the run at its limit until
  /// the step's budget is raised for that run. Versions published while step budgets were money
  /// read as having none (see [`budget`]).
  #[serde(default, deserialize_with = "budget::deserialize")]
  pub budget: Option<TokenBudget>,
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum PipelineError {
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
}

/// Ordered steps forming an acyclic dependency graph. A draft may have no steps yet;
/// publishing requires at least one (see [`crate::resource::ResourceBody::check_publishable`]).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "PipelineFields")]
pub struct Pipeline {
  /// What the pipeline is for, as the user wrote it; may be empty.
  #[serde(default, skip_serializing_if = "String::is_empty")]
  description: String,
  /// The sources the pipeline and its steps list; versions saved before sources existed have none.
  #[serde(default, skip_serializing_if = "PipelineSources::is_empty")]
  sources: PipelineSources,
  steps: Vec<Step>,
  /// The Deliver step it ends in; versions saved before delivery existed, and pipelines whose runs a person merges
  /// from the task, have none.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  deliver: Option<DeliverStep>,
  /// What its runs deliver; versions saved before this existed deliver changes.
  #[serde(default, skip_serializing_if = "Deliverable::is_changes")]
  delivers: Deliverable,
}

#[derive(Deserialize)]
struct PipelineFields {
  #[serde(default)]
  description: String,
  #[serde(default)]
  sources: PipelineSources,
  steps: Vec<Step>,
  #[serde(default)]
  deliver: Option<DeliverStep>,
  #[serde(default)]
  delivers: Deliverable,
}

/// Why a stored pipeline body is not a pipeline.
#[derive(Debug, Error)]
enum PipelineFieldsError {
  #[error(transparent)]
  Pipeline(#[from] PipelineError),
  #[error(transparent)]
  Sources(#[from] PipelineSourcesError),
}

impl TryFrom<PipelineFields> for Pipeline {
  type Error = PipelineFieldsError;

  fn try_from(fields: PipelineFields) -> Result<Self, PipelineFieldsError> {
    Ok(
      Self::new(fields.steps)?
        .with_description(fields.description)
        .with_deliver(fields.deliver)
        .with_delivers(fields.delivers)
        .with_sources(fields.sources)?,
    )
  }
}

impl Pipeline {
  pub fn new(steps: Vec<Step>) -> Result<Self, PipelineError> {
    let graph = Graph::new(&steps)?;
    for step in &steps {
      graph.check_route(step)?;
    }
    Ok(Self {
      description: String::new(),
      sources: PipelineSources::default(),
      steps,
      deliver: None,
      delivers: Deliverable::default(),
    })
  }

  #[must_use]
  pub fn with_description(mut self, description: String) -> Self {
    self.description = description;
    self
  }

  /// This pipeline ending in `deliver`, or in no Deliver step.
  #[must_use]
  pub fn with_deliver(mut self, deliver: Option<DeliverStep>) -> Self {
    self.deliver = deliver;
    self
  }

  /// This pipeline's runs delivering `delivers`.
  #[must_use]
  pub const fn with_delivers(mut self, delivers: Deliverable) -> Self {
    self.delivers = delivers;
    self
  }

  /// This pipeline listing `sources`, whose steps must be its steps.
  pub fn with_sources(mut self, sources: PipelineSources) -> Result<Self, PipelineSourcesError> {
    if let Some(key) = sources.step_keys().find(|key| self.step(key).is_none()) {
      return Err(PipelineSourcesError::UnknownStep(key.clone()));
    }
    self.sources = sources;
    Ok(self)
  }

  pub fn description(&self) -> &str {
    &self.description
  }

  /// The sources the pipeline and its steps list.
  pub fn sources(&self) -> &PipelineSources {
    &self.sources
  }

  /// The Deliver step the pipeline ends in: its runs join the project's merge queue once every step passed.
  pub const fn deliver(&self) -> Option<&DeliverStep> {
    self.deliver.as_ref()
  }

  /// What the pipeline's runs deliver, unless their task says otherwise ([`Deliverable::for_task`]).
  pub const fn delivers(&self) -> Deliverable {
    self.delivers
  }

  /// Steps in the order the user arranged them.
  pub fn steps(&self) -> &[Step] {
    &self.steps
  }

  pub fn step(&self, key: &StepKey) -> Option<&Step> {
    self.steps.iter().find(|step| &step.key == key)
  }

  /// Steps whose agent profile or tool is not chosen yet. Such a pipeline can be kept as a
  /// draft but not published.
  pub fn unassigned_steps(&self) -> impl Iterator<Item = &StepKey> {
    unassigned(&self.steps)
  }

  pub fn into_steps(self) -> Vec<Step> {
    self.steps
  }

  /// The steps `executor` (an agent profile or a tool) runs, with their places from 0.
  pub fn steps_run_by(&self, executor: ResourceId) -> impl Iterator<Item = (usize, &Step)> {
    self
      .steps
      .iter()
      .enumerate()
      .filter(move |(_, step)| step.kind.executor() == Some(executor))
  }

  /// This pipeline with every step `executor` runs left without its profile or tool, as removing that resource
  /// leaves it (`super::removal`); the steps keep everything else.
  #[must_use]
  pub fn without_executor(&self, executor: ResourceId) -> Self {
    let mut pipeline = self.clone();
    for step in &mut pipeline.steps {
      match &mut step.kind {
        StepKind::Agent { profile, .. } if *profile == Some(executor) => *profile = None,
        StepKind::Tool { tool } if *tool == Some(executor) => *tool = None,
        StepKind::Agent { .. } | StepKind::Tool { .. } => {}
      }
    }
    pipeline
  }

  /// The steps in stages: a step's stage is one after the latest stage of the steps it depends
  /// on, so the steps of one stage can run beside each other. Each stage keeps pipeline order.
  pub fn stages(&self) -> Vec<Vec<&Step>> {
    let mut depth: HashMap<&StepKey, usize> = HashMap::with_capacity(self.steps.len());
    // Every pass places at least one more step: the graph is acyclic (see `Pipeline::new`).
    while depth.len() < self.steps.len() {
      for step in &self.steps {
        if depth.contains_key(&step.key) {
          continue;
        }
        let known: Option<Vec<usize>> = step.depends_on.iter().map(|key| depth.get(key).copied()).collect();
        if let Some(known) = known {
          depth.insert(&step.key, known.into_iter().max().map_or(0, |deepest| deepest + 1));
        }
      }
    }
    let count = depth.values().max().map_or(0, |deepest| deepest + 1);
    let mut stages = vec![Vec::new(); count];
    for step in &self.steps {
      stages[depth[&step.key]].push(step);
    }
    stages
  }
}

fn unassigned(steps: &[Step]) -> impl Iterator<Item = &StepKey> {
  steps
    .iter()
    .filter(|step| step.kind.executor().is_none())
    .map(|step| &step.key)
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
