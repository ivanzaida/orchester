//! The configuration a run executes: the published pipeline version and, per step, the
//! effective agent profile version and model. Pinned at run creation so publishing a new
//! version later affects new runs only.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
  id::{ResourceId, VersionId},
  resource::{
    AgentProfile, ChosenResources, Instruction, ModelId, Pipeline, Skill, Step, StepKey, StepKind, TeamTool, Tool,
    Version, VersionNumber,
  },
  source::{RunSources, SourceOrigin, SourcePath},
};

/// A published version a run is bound to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PinnedVersion {
  pub resource: ResourceId,
  pub version: VersionId,
  pub number: VersionNumber,
}

impl PinnedVersion {
  pub fn of<B>(resource: ResourceId, version: &Version<B>) -> Self {
    Self {
      resource,
      version: version.id(),
      number: version.number(),
    }
  }
}

/// Who executes a step in this run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum StepExecutor {
  /// `model` is the step's explicit override when `model_overridden`, otherwise the profile's.
  Agent {
    profile: PinnedVersion,
    model: ModelId,
    model_overridden: bool,
  },
  Tool {
    tool: PinnedVersion,
  },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StepSnapshot {
  key: StepKey,
  executor: StepExecutor,
}

impl StepSnapshot {
  pub fn key(&self) -> &StepKey {
    &self.key
  }

  pub fn executor(&self) -> &StepExecutor {
    &self.executor
  }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SnapshotError {
  /// Publishing rejects such pipelines, so a published version never has one.
  #[error("step `{0}` has no agent profile or tool")]
  UnassignedStep(StepKey),
  #[error("step `{step}` uses agent profile {profile}, which has no published version")]
  UnpublishedProfile { step: StepKey, profile: ResourceId },
  #[error("step `{step}` calls tool {tool}, which has no published version")]
  UnpublishedTool { step: StepKey, tool: ResourceId },
  #[error("the snapshot's steps do not match the pinned pipeline's steps")]
  StepsMismatch,
  #[error("step `{0}` is pinned to an executor its pipeline step does not select")]
  ExecutorMismatch(StepKey),
}

/// The latest published version of each referenced resource, by resource id. A resource that
/// was never published is absent. A snapshot reads only which version of a tool is the latest, so its tools are a local
/// project's ([`PublishedVersions`]) or a team's ([`TeamPublishedVersions`], as the team server holds them).
#[derive(Debug)]
pub struct Published<'a, T> {
  pub profiles: &'a BTreeMap<ResourceId, Version<AgentProfile>>,
  pub tools: &'a BTreeMap<ResourceId, Version<T>>,
  pub instructions: &'a BTreeMap<ResourceId, Version<Instruction>>,
  pub skills: &'a BTreeMap<ResourceId, Version<Skill>>,
}

/// The published versions of a local project, whose tools name their secrets by id.
pub type PublishedVersions<'a> = Published<'a, Tool>;

/// The published versions of a team project, whose tools name their secrets by name.
pub type TeamPublishedVersions<'a> = Published<'a, TeamTool>;

impl<T> Clone for Published<'_, T> {
  fn clone(&self) -> Self {
    *self
  }
}

impl<T> Copy for Published<'_, T> {}

/// Everything a run pins: pipeline, per-step executors, the instructions, tools and skills its agents get, and the
/// sources its agents may read outside the worktree without asking.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "SnapshotFields")]
pub struct RunSnapshot {
  pipeline: PinnedVersion,
  body: Pipeline,
  steps: Vec<StepSnapshot>,
  instructions: Vec<PinnedVersion>,
  tools: Vec<PinnedVersion>,
  skills: Vec<PinnedVersion>,
  #[serde(default, skip_serializing_if = "RunSources::is_empty")]
  sources: RunSources,
}

#[derive(Deserialize)]
struct SnapshotFields {
  pipeline: PinnedVersion,
  body: Pipeline,
  steps: Vec<StepSnapshot>,
  instructions: Vec<PinnedVersion>,
  tools: Vec<PinnedVersion>,
  /// Runs created before skills existed pinned none.
  #[serde(default)]
  skills: Vec<PinnedVersion>,
  /// Runs created before sources existed snapshotted none.
  #[serde(default)]
  sources: RunSources,
}

impl TryFrom<SnapshotFields> for RunSnapshot {
  type Error = SnapshotError;

  fn try_from(fields: SnapshotFields) -> Result<Self, SnapshotError> {
    if fields.steps.len() != fields.body.steps().len() {
      return Err(SnapshotError::StepsMismatch);
    }
    for (step, pinned) in fields.body.steps().iter().zip(&fields.steps) {
      check_executor(step, pinned)?;
    }
    Ok(Self {
      pipeline: fields.pipeline,
      body: fields.body,
      steps: fields.steps,
      instructions: fields.instructions,
      tools: fields.tools,
      skills: fields.skills,
      sources: fields.sources,
    })
  }
}

fn check_executor(step: &Step, pinned: &StepSnapshot) -> Result<(), SnapshotError> {
  if step.key != pinned.key {
    return Err(SnapshotError::StepsMismatch);
  }
  let matches = match (&step.kind, &pinned.executor) {
    (
      StepKind::Agent { profile, model, .. },
      StepExecutor::Agent {
        profile: pinned_profile,
        model: pinned_model,
        model_overridden,
      },
    ) => {
      *profile == Some(pinned_profile.resource)
        && *model_overridden == model.is_some()
        && model.as_ref().is_none_or(|model| model == pinned_model)
    }
    (StepKind::Tool { tool }, StepExecutor::Tool { tool: pinned_tool }) => *tool == Some(pinned_tool.resource),
    _ => false,
  };
  if matches {
    Ok(())
  } else {
    Err(SnapshotError::ExecutorMismatch(step.key.clone()))
  }
}

impl RunSnapshot {
  /// A snapshot without sources from its parts, checked as a stored one is: the steps must be the pipeline's, each
  /// pinned to the executor its pipeline step selects.
  pub(super) fn from_parts(
    (pipeline, body): (PinnedVersion, Pipeline),
    steps: Vec<StepSnapshot>,
    instructions: Vec<PinnedVersion>,
    tools: Vec<PinnedVersion>,
    skills: Vec<PinnedVersion>,
  ) -> Result<Self, SnapshotError> {
    Self::try_from(SnapshotFields {
      pipeline,
      body,
      steps,
      instructions,
      tools,
      skills,
      sources: RunSources::default(),
    })
  }
  /// Resolves `version` of the pipeline `pipeline` against the published versions it
  /// references. Every step's profile or tool must be published; the published instructions, skills and tools the
  /// profiles of its steps choose are pinned alongside, each once (`docs/design/team-configuration.md`, "Decided
  /// 2026-11-09"): nothing else reaches a local run's agents. Which step's agent gets which is decided per invocation,
  /// from its profile version ([`crate::run::GivenBy::Profile`]).
  pub fn build<T>(
    pipeline: ResourceId,
    version: &Version<Pipeline>,
    published: Published<'_, T>,
  ) -> Result<Self, SnapshotError> {
    let body = version.body();
    let steps = body
      .steps()
      .iter()
      .map(|step| resolve_step(step, published))
      .collect::<Result<Vec<_>, _>>()?;
    let profiles: Vec<&AgentProfile> = body
      .steps()
      .iter()
      .filter_map(|step| match &step.kind {
        StepKind::Agent {
          profile: Some(profile), ..
        } => published.profiles.get(profile).map(Version::body),
        _ => None,
      })
      .collect();
    let instructions = chosen(&profiles, AgentProfile::instruction_resources, published.instructions)
      .into_values()
      .collect();
    let skills = chosen(&profiles, AgentProfile::skills, published.skills)
      .into_values()
      .collect();
    let tools = pinned_tools(&steps, chosen(&profiles, AgentProfile::tools, published.tools));
    Ok(Self {
      pipeline: PinnedVersion::of(pipeline, version),
      body: body.clone(),
      steps,
      instructions,
      tools,
      skills,
      sources: RunSources::default(),
    })
  }

  /// This snapshot with the run's sources: the union of the project's (`project`), the task's (`task`), the pinned
  /// pipeline's and its steps', as they are when the run starts.
  #[must_use]
  pub fn with_sources(mut self, project: &[SourcePath], task: &[SourcePath]) -> Self {
    let listed = self.body.sources();
    let steps = self.body.steps().iter().flat_map(|step| {
      listed
        .step(&step.key)
        .iter()
        .map(|path| (path, SourceOrigin::Step { step: step.key.clone() }))
    });
    let sources = RunSources::union(
      project
        .iter()
        .map(|path| (path, SourceOrigin::Project))
        .chain(task.iter().map(|path| (path, SourceOrigin::Task)))
        .chain(listed.pipeline().iter().map(|path| (path, SourceOrigin::Pipeline)))
        .chain(steps),
    );
    self.sources = sources;
    self
  }

  /// The folders and files every agent of the run may read outside its worktree without asking.
  pub const fn sources(&self) -> &RunSources {
    &self.sources
  }

  pub const fn pipeline(&self) -> PinnedVersion {
    self.pipeline
  }

  /// The pinned pipeline body: steps, dependencies, gates and failure routes.
  pub const fn body(&self) -> &Pipeline {
    &self.body
  }

  /// Executors in pipeline step order.
  pub fn steps(&self) -> &[StepSnapshot] {
    &self.steps
  }

  pub fn step(&self, key: &StepKey) -> Option<&StepSnapshot> {
    self.steps.iter().find(|step| &step.key == key)
  }

  pub fn instructions(&self) -> &[PinnedVersion] {
    &self.instructions
  }

  pub fn tools(&self) -> &[PinnedVersion] {
    &self.tools
  }

  /// The skills that reach an agent of the run, in resource id order.
  pub fn skills(&self) -> &[PinnedVersion] {
    &self.skills
  }

  /// Whether the run pins any version of `resource`: its pipeline, a step's profile or tool, or an instruction, tool
  /// or skill that reaches its agents.
  pub fn pins(&self, resource: ResourceId) -> bool {
    let executors = self.steps.iter().map(|step| match &step.executor {
      StepExecutor::Agent { profile, .. } => profile,
      StepExecutor::Tool { tool } => tool,
    });
    std::iter::once(&self.pipeline)
      .chain(executors)
      .chain(&self.instructions)
      .chain(&self.tools)
      .chain(&self.skills)
      .any(|pinned| pinned.resource == resource)
  }

  /// `key` and every step that depends on it, directly or transitively.
  pub fn downstream_of(&self, key: &StepKey) -> BTreeSet<StepKey> {
    let mut reached = BTreeSet::from([key.clone()]);
    let mut grew = true;
    while grew {
      grew = false;
      for step in self.body.steps() {
        if !reached.contains(&step.key) && step.depends_on.iter().any(|dependency| reached.contains(dependency)) {
          reached.insert(step.key.clone());
          grew = true;
        }
      }
    }
    reached
  }
}

fn resolve_step<T>(step: &Step, published: Published<'_, T>) -> Result<StepSnapshot, SnapshotError> {
  let unassigned = || SnapshotError::UnassignedStep(step.key.clone());
  let executor = match &step.kind {
    StepKind::Agent { profile, model, .. } => {
      let profile = profile.as_ref().ok_or_else(unassigned)?;
      let version = published
        .profiles
        .get(profile)
        .ok_or_else(|| SnapshotError::UnpublishedProfile {
          step: step.key.clone(),
          profile: *profile,
        })?;
      StepExecutor::Agent {
        profile: PinnedVersion::of(*profile, version),
        model: model.clone().unwrap_or_else(|| version.body().model().clone()),
        model_overridden: model.is_some(),
      }
    }
    StepKind::Tool { tool } => {
      let tool = tool.as_ref().ok_or_else(unassigned)?;
      let version = published
        .tools
        .get(tool)
        .ok_or_else(|| SnapshotError::UnpublishedTool {
          step: step.key.clone(),
          tool: *tool,
        })?;
      StepExecutor::Tool {
        tool: PinnedVersion::of(*tool, version),
      }
    }
  };
  Ok(StepSnapshot {
    key: step.key.clone(),
    executor,
  })
}

/// The latest published version of each resource one of `profiles` chooses in `list`, by resource id; one that has no
/// published version is not pinned.
fn chosen<B>(
  profiles: &[&AgentProfile],
  list: impl Fn(&AgentProfile) -> &ChosenResources,
  published: &BTreeMap<ResourceId, Version<B>>,
) -> BTreeMap<ResourceId, PinnedVersion> {
  profiles
    .iter()
    .flat_map(|profile| list(profile).ids().iter().copied())
    .filter_map(|id| published.get(&id).map(|version| (id, PinnedVersion::of(id, version))))
    .collect()
}

/// Tools called by tool steps plus the tools the steps' profiles choose, each once.
fn pinned_tools(steps: &[StepSnapshot], chosen: BTreeMap<ResourceId, PinnedVersion>) -> Vec<PinnedVersion> {
  let mut tools: BTreeMap<ResourceId, PinnedVersion> = steps
    .iter()
    .filter_map(|step| match step.executor {
      StepExecutor::Tool { tool } => Some((tool.resource, tool)),
      StepExecutor::Agent { .. } => None,
    })
    .collect();
  for (id, pinned) in chosen {
    tools.entry(id).or_insert(pinned);
  }
  tools.into_values().collect()
}
