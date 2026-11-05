//! The complete resolved inputs of one execution, built on the run's [`RunSnapshot`]: the pinned pipeline with its
//! body, each step's executor, and the bodies of the profile, instruction, tool and skill versions the run pins, each
//! pinned version with the slug and name it had ([`ResourceLabel`]), so a run executes and recovers without reading
//! the mutable current definitions. Nothing in them is a secret value, an
//! account, a connection or a path of one computer: bodies name secrets by id only, the run's sources
//! ([`crate::source::RunSources`]) and the pipeline's are left out, and profiles go without their computer's bindings
//! ([`ExecutionProfile`]).

use serde::{Deserialize, Serialize};

use super::{
  BundleError, ExecutionProfile,
  canonical::{CanonicalError, bundle_sha256},
  files::{BundleFile, MAX_BUNDLE_FILE_BYTES, SkillFileEntry, SkillFiles},
};
use crate::{
  blob::ContentHash,
  id::{ResourceId, VersionId},
  name::Name,
  permission::AgentPermissions,
  resource::{
    AgentProfile, Instruction, Pipeline, PipelineSources, Skill, Slug, StepKey, StepKind, Tool, ToolAction, Version,
    VersionNumber,
  },
  run::{PinnedVersion, RunSnapshot, StepExecutor, StepSnapshot},
};

/// `{slug, name}`: what a resource was called at a pinned version, as prompts and skill folders name it. Both are
/// checked as a slug and a name are, also when a bundle is read.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceLabel {
  slug: Slug,
  name: Name,
}

impl ResourceLabel {
  pub const fn new(slug: Slug, name: Name) -> Self {
    Self { slug, name }
  }

  pub const fn slug(&self) -> &Slug {
    &self.slug
  }

  pub const fn name(&self) -> &Name {
    &self.name
  }
}

/// `{resource, version, number, label, body}`: a pinned published version with its label and body.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PinnedBody<B> {
  resource: ResourceId,
  version: VersionId,
  number: VersionNumber,
  label: ResourceLabel,
  body: B,
}

impl<B> PinnedBody<B> {
  const fn new(pinned: PinnedVersion, label: ResourceLabel, body: B) -> Self {
    Self {
      resource: pinned.resource,
      version: pinned.version,
      number: pinned.number,
      label,
      body,
    }
  }

  pub const fn pinned(&self) -> PinnedVersion {
    PinnedVersion {
      resource: self.resource,
      version: self.version,
      number: self.number,
    }
  }

  /// The resource's slug and name as the pinned version had them.
  pub const fn label(&self) -> &ResourceLabel {
    &self.label
  }

  pub const fn body(&self) -> &B {
    &self.body
  }
}

/// The published versions a snapshot's pins are looked up in, by version id; older versions as well as the latest.
/// `labels` holds each pinned version's label, the pipeline's too, by version id.
#[derive(Clone, Copy, Debug)]
pub struct PinnedBodies<'a> {
  pub profiles: &'a [Version<AgentProfile>],
  pub instructions: &'a [Version<Instruction>],
  pub tools: &'a [Version<Tool>],
  pub skills: &'a [Version<Skill>],
  pub labels: &'a [(VersionId, ResourceLabel)],
}

/// `{step, permissions}`: what an agent step's agents may do, its profile's permissions narrowed by the step.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResolvedStepPermissions {
  pub step: StepKey,
  pub permissions: AgentPermissions,
}

/// `{pipeline, steps, profiles, instructions, tools, skills}`. Steps keep pipeline order; the pinned versions are in
/// resource id order, one per resource.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "InputsFields")]
pub struct CanonicalExecutionInputs {
  pipeline: PinnedBody<Pipeline>,
  steps: Vec<StepSnapshot>,
  profiles: Vec<PinnedBody<ExecutionProfile>>,
  instructions: Vec<PinnedBody<Instruction>>,
  tools: Vec<PinnedBody<Tool>>,
  skills: Vec<PinnedBody<Skill>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct InputsFields {
  pipeline: PinnedBody<Pipeline>,
  steps: Vec<StepSnapshot>,
  profiles: Vec<PinnedBody<ExecutionProfile>>,
  instructions: Vec<PinnedBody<Instruction>>,
  tools: Vec<PinnedBody<Tool>>,
  skills: Vec<PinnedBody<Skill>>,
}

impl TryFrom<InputsFields> for CanonicalExecutionInputs {
  type Error = BundleError;

  fn try_from(fields: InputsFields) -> Result<Self, BundleError> {
    Self::checked(fields)
  }
}

impl CanonicalExecutionInputs {
  /// The inputs of a run that pinned `snapshot`, with the bodies and labels of its pinned versions from `bodies`.
  pub fn pin(snapshot: &RunSnapshot, bodies: PinnedBodies<'_>) -> Result<Self, BundleError> {
    let body = snapshot
      .body()
      .clone()
      .with_sources(PipelineSources::default())
      .expect("no sources fit every pipeline");
    let profiles = agent_profiles(snapshot.steps())
      .into_iter()
      .map(|pinned| {
        Ok(PinnedBody::new(
          pinned,
          label(bodies.labels, pinned)?,
          ExecutionProfile::of(find(bodies.profiles, pinned)?),
        ))
      })
      .collect::<Result<_, BundleError>>()?;
    let pipeline = snapshot.pipeline();
    Self::checked(InputsFields {
      pipeline: PinnedBody::new(pipeline, label(bodies.labels, pipeline)?, body),
      steps: snapshot.steps().to_vec(),
      profiles,
      instructions: pinned_bodies(snapshot.instructions(), bodies.instructions, bodies.labels)?,
      tools: pinned_bodies(snapshot.tools(), bodies.tools, bodies.labels)?,
      skills: pinned_bodies(snapshot.skills(), bodies.skills, bodies.labels)?,
    })
  }

  fn checked(fields: InputsFields) -> Result<Self, BundleError> {
    if !fields.pipeline.body.sources().is_empty() {
      return Err(BundleError::PipelineSources);
    }
    let inputs = Self {
      pipeline: fields.pipeline,
      steps: fields.steps,
      profiles: sorted(fields.profiles)?,
      instructions: sorted(fields.instructions)?,
      tools: sorted(fields.tools)?,
      skills: sorted(fields.skills)?,
    };
    inputs.parts_snapshot()?;
    if pins(&inputs.profiles) != agent_profiles(&inputs.steps) {
      return Err(BundleError::ProfilesMismatch);
    }
    inputs.check_tools()?;
    inputs.check_files()?;
    Ok(inputs)
  }

  fn parts_snapshot(&self) -> Result<RunSnapshot, BundleError> {
    Ok(RunSnapshot::from_parts(
      (self.pipeline.pinned(), self.pipeline.body.clone()),
      self.steps.clone(),
      pins(&self.instructions),
      pins(&self.tools),
      pins(&self.skills),
    )?)
  }

  fn check_tools(&self) -> Result<(), BundleError> {
    for step in &self.steps {
      if let StepExecutor::Tool { tool } = step.executor()
        && !self.tools.iter().any(|pinned| pinned.pinned() == *tool)
      {
        return Err(BundleError::ToolMissing(step.key().clone()));
      }
    }
    match self.tools.iter().find(|tool| starts_by_path(&tool.body)) {
      Some(tool) => Err(BundleError::MachinePath(tool.resource)),
      None => Ok(()),
    }
  }

  fn check_files(&self) -> Result<(), BundleError> {
    for skill in &self.skills {
      if let Some(file) = skill.body.files().iter().find(|file| file.size > MAX_BUNDLE_FILE_BYTES) {
        return Err(BundleError::FileTooLarge {
          skill: skill.resource,
          path: file.path.clone(),
          size: file.size,
        });
      }
    }
    Ok(())
  }

  /// The pinned pipeline; its body lists no sources.
  pub const fn pipeline(&self) -> &PinnedBody<Pipeline> {
    &self.pipeline
  }

  /// Each step's executor, in pipeline step order.
  pub fn steps(&self) -> &[StepSnapshot] {
    &self.steps
  }

  pub fn profiles(&self) -> &[PinnedBody<ExecutionProfile>] {
    &self.profiles
  }

  pub fn instructions(&self) -> &[PinnedBody<Instruction>] {
    &self.instructions
  }

  pub fn tools(&self) -> &[PinnedBody<Tool>] {
    &self.tools
  }

  pub fn skills(&self) -> &[PinnedBody<Skill>] {
    &self.skills
  }

  /// The run snapshot these inputs hold, without sources: the computer that runs them adds its own.
  pub fn snapshot(&self) -> RunSnapshot {
    self
      .parts_snapshot()
      .expect("the inputs were checked as a snapshot when they were made")
  }

  /// Every resource the inputs pin a version of, once, in id order.
  pub fn resources(&self) -> Vec<ResourceId> {
    let mut resources: Vec<ResourceId> = std::iter::once(self.pipeline.resource)
      .chain(self.profiles.iter().map(|pinned| pinned.resource))
      .chain(self.instructions.iter().map(|pinned| pinned.resource))
      .chain(self.tools.iter().map(|pinned| pinned.resource))
      .chain(self.skills.iter().map(|pinned| pinned.resource))
      .collect();
    resources.sort();
    resources.dedup();
    resources
  }

  /// The label pinned resource `resource` had at its pinned version; `None` for a resource the inputs do not pin.
  pub fn label(&self, resource: ResourceId) -> Option<&ResourceLabel> {
    std::iter::once((self.pipeline.resource, &self.pipeline.label))
      .chain(self.profiles.iter().map(|pinned| (pinned.resource, &pinned.label)))
      .chain(self.instructions.iter().map(|pinned| (pinned.resource, &pinned.label)))
      .chain(self.tools.iter().map(|pinned| (pinned.resource, &pinned.label)))
      .chain(self.skills.iter().map(|pinned| (pinned.resource, &pinned.label)))
      .find(|(pinned, _)| *pinned == resource)
      .map(|(_, label)| label)
  }

  /// Each agent step's permissions: its profile's narrowed by the step, in pipeline step order.
  pub fn step_permissions(&self) -> Vec<ResolvedStepPermissions> {
    self
      .pipeline
      .body
      .steps()
      .iter()
      .zip(&self.steps)
      .filter_map(|(step, pinned)| {
        let StepKind::Agent {
          repository,
          permissions,
          ..
        } = &step.kind
        else {
          return None;
        };
        let StepExecutor::Agent { profile, .. } = pinned.executor() else {
          return None;
        };
        let body = self.profiles.iter().find(|pinned| pinned.pinned() == *profile)?;
        Some(ResolvedStepPermissions {
          step: step.key.clone(),
          permissions: body.body.permissions().narrowed(*repository, permissions),
        })
      })
      .collect()
  }

  /// Each pinned skill's reference files, in resource id order.
  pub fn skill_files(&self) -> Vec<SkillFiles> {
    self
      .skills
      .iter()
      .map(|skill| SkillFiles {
        resource_id: skill.resource,
        files: skill
          .body
          .files()
          .iter()
          .map(|file| SkillFileEntry {
            path: file.path.clone(),
            sha256: file.content.clone(),
            size_bytes: file.size,
          })
          .collect(),
      })
      .collect()
  }

  /// Every file the inputs name, once, in hash order.
  pub fn files(&self) -> Vec<BundleFile> {
    let mut files: Vec<BundleFile> = self
      .skill_files()
      .iter()
      .flat_map(|skill| skill.files.iter().map(SkillFileEntry::file))
      .collect();
    files.sort();
    files.dedup();
    files
  }

  /// The hash of these inputs' canonical bytes ([`super::canonical`]).
  pub fn sha256(&self) -> Result<ContentHash, CanonicalError> {
    bundle_sha256(self)
  }
}

fn find<B>(versions: &[Version<B>], pinned: PinnedVersion) -> Result<&B, BundleError> {
  versions
    .iter()
    .find(|version| version.id() == pinned.version && version.number() == pinned.number)
    .map(Version::body)
    .ok_or(BundleError::MissingVersion {
      resource: pinned.resource,
      number: pinned.number,
    })
}

fn label(labels: &[(VersionId, ResourceLabel)], pinned: PinnedVersion) -> Result<ResourceLabel, BundleError> {
  labels
    .iter()
    .find(|(version, _)| *version == pinned.version)
    .map(|(_, label)| label.clone())
    .ok_or(BundleError::MissingLabel {
      resource: pinned.resource,
      number: pinned.number,
    })
}

fn pins<B>(list: &[PinnedBody<B>]) -> Vec<PinnedVersion> {
  list.iter().map(PinnedBody::pinned).collect()
}

fn pinned_bodies<B: Clone>(
  pins: &[PinnedVersion],
  versions: &[Version<B>],
  labels: &[(VersionId, ResourceLabel)],
) -> Result<Vec<PinnedBody<B>>, BundleError> {
  pins
    .iter()
    .map(|&pinned| {
      Ok(PinnedBody::new(
        pinned,
        label(labels, pinned)?,
        find(versions, pinned)?.clone(),
      ))
    })
    .collect()
}

/// `list` in resource id order; a resource pinned twice is refused.
fn sorted<B>(mut list: Vec<PinnedBody<B>>) -> Result<Vec<PinnedBody<B>>, BundleError> {
  list.sort_by_key(|pinned| pinned.resource);
  match list.windows(2).find(|pair| pair[0].resource == pair[1].resource) {
    Some(pair) => Err(BundleError::Duplicate(pair[0].resource)),
    None => Ok(list),
  }
}

/// The profile versions the agent steps pin, once each, in resource id order.
fn agent_profiles(steps: &[StepSnapshot]) -> Vec<PinnedVersion> {
  let mut pinned: Vec<PinnedVersion> = steps
    .iter()
    .filter_map(|step| match step.executor() {
      StepExecutor::Agent { profile, .. } => Some(*profile),
      StepExecutor::Tool { .. } => None,
    })
    .collect();
  pinned.sort_by_key(|pinned| (pinned.resource, pinned.version));
  pinned.dedup();
  pinned
}

/// Whether a command tool starts its program by an absolute path (`/usr/bin/x`, `\tools\x`, `C:\x`), read the same on
/// every platform so a bundle is refused or taken alike everywhere.
fn starts_by_path(tool: &Tool) -> bool {
  let ToolAction::Command(command) = tool.action() else {
    return false;
  };
  let program = command.program().as_str();
  let mut chars = program.chars();
  let drive = chars.next().is_some_and(|letter| letter.is_ascii_alphabetic()) && chars.next() == Some(':');
  program.starts_with(['/', '\\']) || drive
}
