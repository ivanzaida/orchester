//! The execution bundle (S8): the immutable manifest of everything one authorized execution runs with, which the
//! server builds when it starts work under a claim and the desktop reads by its hash ([`BundleRef`]). Resource versions
//! and file hashes bind the complete resolved inputs; nothing is "latest" when the run executes. A bundle holds no
//! credential, account, connection or path of one computer: secrets stay in the local credential store, and each
//! profile's CLI is bound on the executing computer from [`LocalBinding`], which names the profile by its server
//! resource id and holds no id of a computer's connection, MCP server or secret. The team MCP servers a profile names
//! are in the bundle's inputs by server resource id, with names only: each computer sets a program's path and the
//! values itself.
//!
//! Its hash is the SHA-256 of its canonical bytes ([`orchester_core::run::bundle::canonical`]). The server hashes the
//! bundle it built and never accepts one a client wrote. The work's module, its branch and the base commit it starts
//! from are S7's (`module_id`, `branch`, `base_commit`); the work itself is not in the bundle: a bundle binds to its
//! claim, and S7's `work.start` attaches the work to it. A task in no module has a bundle without `module_id` and
//! `branch` ([`MODULE_LESS_BUNDLE_SCHEMA`]): its run starts on the branch its computer has checked out, and merges
//! into that branch.
//!
//! Reads: the bundle read answers an `ExecutionBundle`; a bundle file read answers one file's raw bytes by its
//! `sha256`, at most [`MAX_BUNDLE_FILE_BYTES`], which the desktop checks with [`ExecutionBundle::check_file`].

use std::num::NonZeroU32;

pub use orchester_core::run::bundle::{
  MAX_BUNDLE_FILE_BYTES, ResolvedStepPermissions, ResourceLabel, SkillFileEntry, SkillFiles,
};
/// `{title, brief, criteria}`: the task the run executes, as it was at the bundle's `definition_revision`; a run
/// keeps it with its inputs.
pub use orchester_core::run::inputs::TaskAtStart;
use orchester_core::{
  blob::ContentHash,
  connection::AgentProvider,
  id::{ClaimId, ModuleId, ProjectId, ResourceId},
  name::Name,
  resource::{Deliverable, ModelId, VersionNumber},
  run::{
    BranchName, CommitId,
    bundle::{BundleFile, CanonicalError, CanonicalExecutionInputs, bundle_sha256},
  },
  sharing::TaskRevision,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::{BundleRef, Subject};
use crate::SharedLimits;

/// The version of the shape of a bundle under a module, as [`BundleRef::schema_version`] names it. 2 added the task's
/// number and each pinned version's label; 3 added the team MCP server versions the pinned profiles name, and the
/// server ids a team profile lists. A side that reads no version of a bundle refuses it
/// ([`ExecutionBundle::reads`]).
pub const EXECUTION_BUNDLE_SCHEMA: u32 = 3;

/// The version of a bundle whose task is in no module: schema 3 without `module_id` and `branch`. A bundle under a
/// module stays schema 3, byte for byte, so a desktop that reads only 3 still runs every task in a module and
/// refuses one in none as built in a schema it does not read.
pub const MODULE_LESS_BUNDLE_SCHEMA: u32 = 4;

/// `{resource_id, version}`: a pinned published version.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceVersionRef {
  pub resource_id: ResourceId,
  pub version: VersionNumber,
}

/// `{role, resource_id, version}`: a pinned agent profile version.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProfileVersionRef {
  pub role: Name,
  pub resource_id: ResourceId,
  pub version: VersionNumber,
}

/// `{resource_id, role, cli, model}`: what the executing computer binds a pinned profile to, its agent CLI and model;
/// which of its accounts and connections run it stays on that computer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LocalBinding {
  pub resource_id: ResourceId,
  pub role: Name,
  pub cli: AgentProvider,
  pub model: ModelId,
}

/// The module a work is under, with the module's branch, which the work's runs merge into and only the module's
/// delivery takes to base.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BundleModule {
  pub module_id: ModuleId,
  pub branch: BranchName,
}

/// What the server decides when it starts the work, besides the resolved inputs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BundleStart {
  pub project_id: ProjectId,
  pub subject: Subject,
  pub claim_id: ClaimId,
  /// The module the work is under; `None` for a task in no module, whose run starts on the branch the executing
  /// computer has checked out and merges into it.
  pub module: Option<BundleModule>,
  /// The commit of the base branch the work starts from.
  pub base_commit: CommitId,
  /// The task's revision when the work started.
  pub definition_revision: TaskRevision,
  /// The task's number in its project, the `7` of `ORC-7` that names the run and its branch.
  pub task_number: NonZeroU32,
  pub task: TaskAtStart,
  /// The run's token budget and time limit; `{}` for none.
  pub limits: SharedLimits,
  /// What the run delivers: the task's own choice, else its pipeline's ([`Deliverable::for_task`]).
  pub delivery: Deliverable,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ExecutionBundleError {
  #[error("bundle schema {0} is neither {EXECUTION_BUNDLE_SCHEMA} nor {MODULE_LESS_BUNDLE_SCHEMA}")]
  Schema(u32),
  #[error("agent profile {0} has no local binding")]
  MissingBinding(ResourceId),
  /// A field the inputs decide does not say what they say.
  #[error("the bundle's `{0}` does not match its inputs")]
  Inconsistent(&'static str),
}

/// Why bytes a bundle file read answered are not the file asked for.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum BundleFileError {
  #[error("file {0} is not one of the bundle's files")]
  NotInBundle(ContentHash),
  #[error("file {0} does not have the size and hash the bundle names")]
  Mismatch(ContentHash),
}

/// The bundle, as [`super`]'s module docs and `docs/cloud-api.md` give its wire shape. The version refs, permissions,
/// files and skill file manifests are derived from `inputs`; a read whose fields do not match them is refused.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
  try_from = "super::bundle_wire::ExecutionBundleWire",
  into = "super::bundle_wire::ExecutionBundleWire"
)]
pub struct ExecutionBundle {
  pub(super) start: BundleStart,
  pub(super) bindings: Vec<LocalBinding>,
  pub(super) inputs: CanonicalExecutionInputs,
}

impl ExecutionBundle {
  /// The bundle of `inputs` started as `start`, each pinned profile bound to its CLI in `clis` (by resource id).
  pub fn new(
    start: BundleStart,
    inputs: CanonicalExecutionInputs,
    clis: &[(ResourceId, AgentProvider)],
  ) -> Result<Self, ExecutionBundleError> {
    let bindings = inputs
      .profiles()
      .iter()
      .map(|profile| {
        let resource_id = profile.pinned().resource;
        let cli = clis
          .iter()
          .find(|(id, _)| *id == resource_id)
          .map(|(_, cli)| *cli)
          .ok_or(ExecutionBundleError::MissingBinding(resource_id))?;
        Ok(LocalBinding {
          resource_id,
          role: profile.body().role().clone(),
          cli,
          model: profile.body().model().clone(),
        })
      })
      .collect::<Result<_, ExecutionBundleError>>()?;
    Ok(Self {
      start,
      bindings,
      inputs,
    })
  }

  pub const fn start(&self) -> &BundleStart {
    &self.start
  }

  /// The schema this bundle is written in: [`EXECUTION_BUNDLE_SCHEMA`] under a module,
  /// [`MODULE_LESS_BUNDLE_SCHEMA`] in none.
  pub const fn schema_version(&self) -> u32 {
    if self.start.module.is_some() {
      EXECUTION_BUNDLE_SCHEMA
    } else {
      MODULE_LESS_BUNDLE_SCHEMA
    }
  }

  /// Whether this side reads a bundle built in `schema`.
  pub const fn reads(schema: u32) -> bool {
    schema == EXECUTION_BUNDLE_SCHEMA || schema == MODULE_LESS_BUNDLE_SCHEMA
  }

  /// One per pinned profile, in resource id order.
  pub fn bindings(&self) -> &[LocalBinding] {
    &self.bindings
  }

  pub const fn inputs(&self) -> &CanonicalExecutionInputs {
    &self.inputs
  }

  /// The SHA-256 of the bundle's canonical bytes.
  pub fn sha256(&self) -> Result<ContentHash, CanonicalError> {
    bundle_sha256(self)
  }

  /// What a start effect names this bundle by.
  pub fn bundle_ref(&self) -> Result<BundleRef, CanonicalError> {
    Ok(BundleRef {
      sha256: self.sha256()?,
      schema_version: self.schema_version(),
    })
  }

  /// Whether `bytes`, read for the file `sha256`, are that file of this bundle.
  pub fn check_file(&self, sha256: &ContentHash, bytes: &[u8]) -> Result<(), BundleFileError> {
    let file = self
      .inputs
      .files()
      .into_iter()
      .find(|file: &BundleFile| &file.sha256 == sha256)
      .ok_or_else(|| BundleFileError::NotInBundle(sha256.clone()))?;
    if file.matches(bytes) {
      Ok(())
    } else {
      Err(BundleFileError::Mismatch(sha256.clone()))
    }
  }
}
