//! The configuration a run executes: the published pipeline version and, per step, the
//! effective agent profile version and model. Pinned at run creation so publishing a new
//! version later affects new runs only.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
  id::{ResourceId, VersionId},
  resource::{
    AgentProfile, Exclusion, Instruction, InstructionTarget, ModelId, Pipeline, Skill, Step, StepKey, StepKind, Tool,
    Version, VersionNumber,
  },
};

/// A published version a run is bound to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
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
/// was never published is absent.
#[derive(Clone, Copy, Debug)]
pub struct PublishedVersions<'a> {
  pub profiles: &'a BTreeMap<ResourceId, Version<AgentProfile>>,
  pub tools: &'a BTreeMap<ResourceId, Version<Tool>>,
  pub instructions: &'a BTreeMap<ResourceId, Version<Instruction>>,
  pub skills: &'a BTreeMap<ResourceId, Version<Skill>>,
}

/// Everything a run pins: pipeline, per-step executors, and the instructions, tools and skills that
/// apply to it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "SnapshotFields")]
pub struct RunSnapshot {
  pipeline: PinnedVersion,
  body: Pipeline,
  steps: Vec<StepSnapshot>,
  instructions: Vec<PinnedVersion>,
  tools: Vec<PinnedVersion>,
  skills: Vec<PinnedVersion>,
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
  /// Resolves `version` of the pipeline `pipeline` against the published versions it
  /// references. Every step's profile or tool must be published; instructions and tools that
  /// apply to the pipeline are pinned alongside.
  pub fn build(
    pipeline: ResourceId,
    version: &Version<Pipeline>,
    published: PublishedVersions<'_>,
  ) -> Result<Self, SnapshotError> {
    let body = version.body();
    let steps = body
      .steps()
      .iter()
      .map(|step| resolve_step(step, published))
      .collect::<Result<Vec<_>, _>>()?;
    let instructions = published
      .instructions
      .iter()
      .filter(|(_, instruction)| applies(pipeline, body, published, |target| instruction.body().reaches(target)))
      .map(|(&id, instruction)| PinnedVersion::of(id, instruction))
      .collect();
    let skills = published
      .skills
      .iter()
      .filter(|(_, skill)| applies(pipeline, body, published, |target| skill.body().reaches(target)))
      .map(|(&id, skill)| PinnedVersion::of(id, skill))
      .collect();
    let tools = pinned_tools(pipeline, body, &steps, published);
    Ok(Self {
      pipeline: PinnedVersion::of(pipeline, version),
      body: body.clone(),
      steps,
      instructions,
      tools,
      skills,
    })
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

fn resolve_step(step: &Step, published: PublishedVersions<'_>) -> Result<StepSnapshot, SnapshotError> {
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

/// Whether an instruction or a skill reaches an agent of this pipeline (at some event): `reaches`
/// admits a step whose published profile runs it. Which event an instruction is injected at, and
/// which agents get a skill, is decided per invocation, from the pinned version.
fn applies(
  pipeline: ResourceId,
  body: &Pipeline,
  published: PublishedVersions<'_>,
  reaches: impl Fn(InstructionTarget<'_>) -> Result<(), Exclusion>,
) -> bool {
  body.steps().iter().any(|step| {
    let StepKind::Agent {
      profile: Some(profile), ..
    } = &step.kind
    else {
      return false;
    };
    published.profiles.get(profile).is_some_and(|version| {
      let target = InstructionTarget {
        pipeline,
        step: &step.key,
        profile: *profile,
        role: version.body().role(),
      };
      reaches(target).is_ok()
    })
  })
}

/// Tools called by tool steps plus published tools whose access policy admits a step of this
/// pipeline, each once.
fn pinned_tools(
  pipeline: ResourceId,
  body: &Pipeline,
  steps: &[StepSnapshot],
  published: PublishedVersions<'_>,
) -> Vec<PinnedVersion> {
  let mut tools: BTreeMap<ResourceId, PinnedVersion> = steps
    .iter()
    .filter_map(|step| match step.executor {
      StepExecutor::Tool { tool } => Some((tool.resource, tool)),
      StepExecutor::Agent { .. } => None,
    })
    .collect();
  for (&id, version) in published.tools {
    let allowed = body.steps().iter().any(|step| {
      version
        .body()
        .access()
        .is_some_and(|access| access.allows(pipeline, &step.key))
    });
    if allowed {
      tools.entry(id).or_insert_with(|| PinnedVersion::of(id, version));
    }
  }
  tools.into_values().collect()
}
