//! The bundle's wire shape, in the field order of its canonical bytes:
//! `{schema_version, project_id, subject, claim_id, module_id, branch, base_commit, definition_revision, task,
//! pipeline, profiles, permissions, limits, delivery, resources, files, skill_files, bindings, inputs}`. The refs,
//! permissions, files and skill file manifests are what the inputs decide; collections whose order means nothing are
//! read in any order and kept sorted.

use orchester_core::{
  id::{ClaimId, ModuleId, ProjectId},
  resource::Deliverable,
  run::{
    BranchName, CommitId,
    bundle::{BundleFile, CanonicalExecutionInputs, PinnedBody, ResolvedStepPermissions, SkillFiles},
  },
  sharing::TaskRevision,
};
use serde::{Deserialize, Serialize};

use super::{
  BundleStart, EXECUTION_BUNDLE_SCHEMA, ExecutionBundle, ExecutionBundleError, LocalBinding, ProfileVersionRef,
  ResourceVersionRef, Subject, TaskAtStart,
};
use crate::SharedLimits;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ExecutionBundleWire {
  schema_version: u32,
  project_id: ProjectId,
  subject: Subject,
  claim_id: ClaimId,
  module_id: ModuleId,
  branch: BranchName,
  base_commit: CommitId,
  definition_revision: TaskRevision,
  task: TaskAtStart,
  pipeline: ResourceVersionRef,
  profiles: Vec<ProfileVersionRef>,
  permissions: Vec<ResolvedStepPermissions>,
  limits: SharedLimits,
  delivery: Deliverable,
  resources: Vec<ResourceVersionRef>,
  files: Vec<BundleFile>,
  skill_files: Vec<SkillFiles>,
  bindings: Vec<LocalBinding>,
  inputs: CanonicalExecutionInputs,
}

/// What `inputs` decide of the wire shape.
struct Derived {
  pipeline: ResourceVersionRef,
  profiles: Vec<ProfileVersionRef>,
  permissions: Vec<ResolvedStepPermissions>,
  resources: Vec<ResourceVersionRef>,
  files: Vec<BundleFile>,
  skill_files: Vec<SkillFiles>,
}

fn version_ref<B>(pinned: &PinnedBody<B>) -> ResourceVersionRef {
  ResourceVersionRef {
    resource_id: pinned.pinned().resource,
    version: pinned.pinned().number,
  }
}

impl Derived {
  fn of(inputs: &CanonicalExecutionInputs) -> Self {
    let mut resources: Vec<ResourceVersionRef> = (inputs.instructions().iter().map(version_ref))
      .chain(inputs.tools().iter().map(version_ref))
      .chain(inputs.skills().iter().map(version_ref))
      .collect();
    resources.sort();
    Self {
      pipeline: version_ref(inputs.pipeline()),
      profiles: inputs
        .profiles()
        .iter()
        .map(|profile| ProfileVersionRef {
          role: profile.body().role().clone(),
          resource_id: profile.pinned().resource,
          version: profile.pinned().number,
        })
        .collect(),
      permissions: inputs.step_permissions(),
      resources,
      files: inputs.files(),
      skill_files: inputs.skill_files(),
    }
  }
}

impl From<ExecutionBundle> for ExecutionBundleWire {
  fn from(bundle: ExecutionBundle) -> Self {
    let derived = Derived::of(&bundle.inputs);
    let start = bundle.start;
    Self {
      schema_version: EXECUTION_BUNDLE_SCHEMA,
      project_id: start.project_id,
      subject: start.subject,
      claim_id: start.claim_id,
      module_id: start.module_id,
      branch: start.branch,
      base_commit: start.base_commit,
      definition_revision: start.definition_revision,
      task: start.task,
      pipeline: derived.pipeline,
      profiles: derived.profiles,
      permissions: derived.permissions,
      limits: start.limits,
      delivery: start.delivery,
      resources: derived.resources,
      files: derived.files,
      skill_files: derived.skill_files,
      bindings: bundle.bindings,
      inputs: bundle.inputs,
    }
  }
}

impl TryFrom<ExecutionBundleWire> for ExecutionBundle {
  type Error = ExecutionBundleError;

  fn try_from(mut wire: ExecutionBundleWire) -> Result<Self, ExecutionBundleError> {
    if wire.schema_version != EXECUTION_BUNDLE_SCHEMA {
      return Err(ExecutionBundleError::Schema(wire.schema_version));
    }
    let derived = Derived::of(&wire.inputs);
    wire.profiles.sort_by_key(|profile| profile.resource_id);
    wire.resources.sort();
    wire.files.sort();
    wire.skill_files.sort_by_key(|skill| skill.resource_id);
    wire.bindings.sort_by_key(|binding| binding.resource_id);
    let checks = [
      (wire.pipeline == derived.pipeline, "pipeline"),
      (wire.profiles == derived.profiles, "profiles"),
      (wire.permissions == derived.permissions, "permissions"),
      (wire.resources == derived.resources, "resources"),
      (wire.files == derived.files, "files"),
      (wire.skill_files == derived.skill_files, "skill_files"),
      (binds_profiles(&wire.bindings, &wire.inputs), "bindings"),
    ];
    if let Some((_, field)) = checks.into_iter().find(|(matches, _)| !matches) {
      return Err(ExecutionBundleError::Inconsistent(field));
    }
    Ok(Self {
      start: BundleStart {
        project_id: wire.project_id,
        subject: wire.subject,
        claim_id: wire.claim_id,
        module_id: wire.module_id,
        branch: wire.branch,
        base_commit: wire.base_commit,
        definition_revision: wire.definition_revision,
        task: wire.task,
        limits: wire.limits,
        delivery: wire.delivery,
      },
      bindings: wire.bindings,
      inputs: wire.inputs,
    })
  }
}

/// Whether `bindings` bind each pinned profile once, with its role and model.
fn binds_profiles(bindings: &[LocalBinding], inputs: &CanonicalExecutionInputs) -> bool {
  bindings.len() == inputs.profiles().len()
    && bindings.iter().zip(inputs.profiles()).all(|(binding, profile)| {
      binding.resource_id == profile.pinned().resource
        && &binding.role == profile.body().role()
        && &binding.model == profile.body().model()
    })
}
