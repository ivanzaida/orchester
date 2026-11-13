//! The bundle's wire shape, in the field order of its canonical bytes:
//! `{schema_version, project_id, subject, claim_id, module_id, branch, base_commit, definition_revision, task_number,
//! task, pipeline, profiles, permissions, limits, delivery, resources, files, skill_files, bindings, inputs}`. The
//! refs, permissions, files and skill file manifests are what the inputs decide; collections whose order means nothing
//! are read in any order and kept sorted.

use std::num::NonZeroU32;

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
  BundleModule, BundleStart, EXECUTION_BUNDLE_SCHEMA, ExecutionBundle, ExecutionBundleError, LocalBinding,
  MODULE_LESS_BUNDLE_SCHEMA, ProfileVersionRef, ResourceVersionRef, Subject, TaskAtStart,
};
use crate::SharedLimits;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ExecutionBundleWire {
  schema_version: u32,
  project_id: ProjectId,
  subject: Subject,
  claim_id: ClaimId,
  /// With `branch`, there in schema 3 and left out in schema 4.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  module_id: Option<ModuleId>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  branch: Option<BranchName>,
  base_commit: CommitId,
  definition_revision: TaskRevision,
  task_number: NonZeroU32,
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
      .chain(inputs.mcp_servers().iter().map(version_ref))
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
    let schema_version = bundle.schema_version();
    let start = bundle.start;
    let (module_id, branch) = start.module.map(|module| (module.module_id, module.branch)).unzip();
    Self {
      schema_version,
      project_id: start.project_id,
      subject: start.subject,
      claim_id: start.claim_id,
      module_id,
      branch,
      base_commit: start.base_commit,
      definition_revision: start.definition_revision,
      task_number: start.task_number,
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
    let module = match (wire.schema_version, wire.module_id.take(), wire.branch.take()) {
      (EXECUTION_BUNDLE_SCHEMA, Some(module_id), Some(branch)) => Some(BundleModule { module_id, branch }),
      (MODULE_LESS_BUNDLE_SCHEMA, None, None) => None,
      // Schema 3 is a bundle under a module and schema 4 one in none: the fields say otherwise.
      (EXECUTION_BUNDLE_SCHEMA | MODULE_LESS_BUNDLE_SCHEMA, ..) => {
        return Err(ExecutionBundleError::Inconsistent("module_id"));
      }
      (other, ..) => return Err(ExecutionBundleError::Schema(other)),
    };
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
        module,
        base_commit: wire.base_commit,
        definition_revision: wire.definition_revision,
        task_number: wire.task_number,
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
