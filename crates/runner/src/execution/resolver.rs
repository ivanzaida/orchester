//! Where a run's pinned configuration and its task are read from (team authority §7): one resolver for every step,
//! prompt, tool, instruction, skill, resume, retry, follow-up and run detail. A run created with execution inputs reads
//! them and nothing else: no current task or resource definition, so a team run executes and recovers without them
//! and a publication after its start changes nothing for it. A local run made without inputs reads the published
//! versions it pinned, which never change, and the task as the run started it, as before. Skill files are read by
//! their content hash and checked against it; secret values are never part of what is resolved.

use std::sync::Arc;

use orchester_core::{
  id::{ResourceId, RunId, TaskId},
  name::Name,
  permission::AgentPermissions,
  resource::{AgentProfile, Instruction, ResourceBody, Skill, SkillFile, Slug, StepKey, Tool, VersionNumber},
  run::{PinnedVersion, Run, bundle::file_sha256, inputs::RunInputs},
  store::{BlobStore, ResourceStore},
  task::Task,
};
use orchester_store::{ProjectDb, StoreError};
use thiserror::Error;

/// Why a pinned body, label or file of a run cannot be resolved.
#[derive(Debug, Error)]
pub(super) enum ResolveError {
  #[error(transparent)]
  Store(#[from] StoreError),
  #[error("{what} {resource} no longer exists")]
  Gone { what: &'static str, resource: ResourceId },
  #[error("{what} version {number} is missing")]
  VersionMissing { what: &'static str, number: VersionNumber },
  #[error("skill file {path} does not have the content its skill names")]
  FileMismatch { path: String },
}

/// What a run's pinned configuration and task are read from.
#[derive(Clone, Debug)]
pub(super) enum RunResolver {
  /// The inputs the run was created with.
  Inputs(Arc<RunInputs>),
  /// The published versions a local run made without inputs pinned.
  Published,
}

impl RunResolver {
  /// The resolver of run `run`: its inputs when it has them.
  pub(super) fn load(db: &ProjectDb, run: RunId) -> Result<Self, StoreError> {
    Ok(
      db.run_inputs(run)?
        .map_or(Self::Published, |inputs| Self::Inputs(Arc::new(inputs))),
    )
  }

  /// The run's inputs when the team server accepted them: its task's definition, status and readiness are the
  /// server's, and this computer never changes them.
  pub(super) fn team(&self) -> Option<&RunInputs> {
    match self {
      Self::Inputs(inputs) if inputs.is_team() => Some(inputs),
      Self::Inputs(_) | Self::Published => None,
    }
  }

  /// The slug and name of pinned resource `resource`, a `what`.
  pub(super) fn label(
    &self,
    db: &ProjectDb,
    resource: ResourceId,
    what: &'static str,
  ) -> Result<(Slug, Name), ResolveError> {
    let gone = || ResolveError::Gone { what, resource };
    match self {
      Self::Inputs(inputs) => inputs
        .bundle()
        .inputs()
        .label(resource)
        .map(|label| (label.slug().clone(), label.name().clone()))
        .ok_or_else(gone),
      Self::Published => db
        .resource(resource)?
        .map(|kept| (kept.slug().clone(), kept.name().clone()))
        .ok_or_else(gone),
    }
  }

  /// The agent profile version `pinned` as the run's agents run it.
  pub(super) fn profile(&self, db: &ProjectDb, pinned: PinnedVersion) -> Result<AgentProfile, ResolveError> {
    self.body(db, pinned, "agent profile", |inputs| inputs.profile(pinned))
  }

  pub(super) fn instruction(&self, db: &ProjectDb, pinned: PinnedVersion) -> Result<Instruction, ResolveError> {
    self.body(db, pinned, "instruction", |inputs| inputs.instruction(pinned).cloned())
  }

  pub(super) fn tool(&self, db: &ProjectDb, pinned: PinnedVersion) -> Result<Tool, ResolveError> {
    self.body(db, pinned, "tool", |inputs| inputs.tool(pinned).cloned())
  }

  pub(super) fn skill(&self, db: &ProjectDb, pinned: PinnedVersion) -> Result<Skill, ResolveError> {
    self.body(db, pinned, "skill", |inputs| {
      inputs
        .bundle()
        .inputs()
        .skills()
        .iter()
        .find(|skill| skill.pinned() == pinned)
        .map(|skill| skill.body().clone())
    })
  }

  fn body<B: ResourceBody + Clone>(
    &self,
    db: &ProjectDb,
    pinned: PinnedVersion,
    what: &'static str,
    from_inputs: impl FnOnce(&RunInputs) -> Option<B>,
  ) -> Result<B, ResolveError> {
    let missing = || ResolveError::VersionMissing {
      what,
      number: pinned.number,
    };
    match self {
      Self::Inputs(inputs) => from_inputs(inputs).ok_or_else(missing),
      Self::Published => db
        .version::<B>(pinned.resource, pinned.number)?
        .map(|version| version.body().clone())
        .ok_or_else(missing),
    }
  }

  /// The bytes of skill file `file`, read by its content hash and checked against it and its size.
  pub(super) fn skill_file(db: &ProjectDb, file: &SkillFile) -> Result<Vec<u8>, ResolveError> {
    let content = db.blob(&file.content)?;
    let size = u64::try_from(content.len()).unwrap_or(u64::MAX);
    if size != file.size || file_sha256(&content) != file.content {
      return Err(ResolveError::FileMismatch {
        path: file.path.as_str().to_owned(),
      });
    }
    Ok(content)
  }

  /// What a person on this computer narrowed the agents of step `step` to, for a run whose inputs say so.
  pub(super) fn narrowed(&self, step: &StepKey) -> Option<&AgentPermissions> {
    match self {
      Self::Inputs(inputs) => inputs.narrowed(step),
      Self::Published => None,
    }
  }

  /// Task `task` as `run` started it: a team run's from its inputs; a local run's as the project database kept it for
  /// the run. `None` when a local run's task is gone.
  pub(super) fn task_at_start(&self, db: &ProjectDb, task: TaskId, run: &Run) -> Result<Option<Task>, StoreError> {
    match self.team() {
      Some(inputs) => Ok(Some(inputs.task(task, db.project().key(), run.created_at()))),
      None => db.task_for_run(task, run.id()),
    }
  }
}
