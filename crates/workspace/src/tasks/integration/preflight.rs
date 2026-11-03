//! The preflight of a merge: what it holds while Git runs, and its refusals, each with the stage that refused and the
//! cause, logged where it refused (`orchester_core::integration::PreflightStage`). A refusal merged nothing and left
//! the branch and checkout as they were.

use std::{path::Path, sync::Arc};

use orchester_core::integration::{PreflightCause, PreflightStage, local::LocalError};
use orchester_exec::local_git::{IntegrationLease, LocalGit, LocalGitError, owned::StoreOwnership, prepare_scratch};

use super::IntegrationProblem;
use crate::{messages::Message, tasks::local_integration::IntegrationError};

/// What a merge holds from its preflight until it ends: the checkout with its Git metadata, the project's integration
/// lease, and the project database against substitution; let go in that order, the reverse of taking them.
pub(crate) struct Held {
  _git: LocalGit,
  _lease: IntegrationLease,
  _store: Arc<StoreOwnership>,
}

/// Takes what a merge in the project at `root` holds, each stage refusing with its cause. The database is checked
/// first, as local integration does, so a file beside it that is not as SQLite left it is told as the database's.
/// Putting back what a merge left, and clearing a merge the app ended, hold the same.
pub(crate) fn hold(root: &Path) -> Result<Held, IntegrationProblem> {
  let store = StoreOwnership::acquire(root).map_err(git_refusal(root, PreflightStage::ProjectDatabase))?;
  let lease = IntegrationLease::acquire(root).map_err(git_refusal(root, PreflightStage::IntegrationLock))?;
  let scratch = prepare_scratch(root, &[".orchester", "local-integration", "legacy"])
    .map_err(git_refusal(root, PreflightStage::ScratchFolder))?;
  let mut git = LocalGit::new(root, &scratch).map_err(git_refusal(root, PreflightStage::CheckoutIdentity))?;
  git
    .own_metadata()
    .map_err(git_refusal(root, PreflightStage::GitMetadata))?;
  Ok(Held {
    _git: git,
    _lease: lease,
    _store: store,
  })
}

/// The refusal of `stage` for `cause`, logged.
fn refusal(stage: PreflightStage, cause: PreflightCause) -> IntegrationProblem {
  tracing::warn!(stage = stage.name(), %cause, "the preflight of a merge refused it; nothing was merged");
  IntegrationProblem::Preflight { stage, cause }
}

/// The refusal of `stage` of a merge in the project at `root` for a local Git error. Only the integration lease
/// another operation holds reads as held.
fn git_refusal(root: &Path, stage: PreflightStage) -> impl FnOnce(LocalGitError) -> IntegrationProblem + '_ {
  move |error| {
    let cause = match error {
      LocalGitError::State(LocalError::ForeignOperation) if stage == PreflightStage::IntegrationLock => {
        PreflightCause::Held
      }
      error => error.preflight_cause(root),
    };
    refusal(stage, cause)
  }
}

/// The refusal of `stage` of a merge in the project at `root` for a local integration error.
pub(crate) fn integration_refusal(
  root: &Path,
  stage: PreflightStage,
) -> impl FnOnce(IntegrationError) -> IntegrationProblem + '_ {
  move |error| {
    let cause = match error {
      IntegrationError::Git(error) => return git_refusal(root, stage)(error),
      IntegrationError::State(error) => PreflightCause::State(error),
      other => PreflightCause::Reason(other.to_string()),
    };
    refusal(stage, cause)
  }
}

/// What to tell of a refusal of `stage` for `cause`: what the stage is, why it refused and that nothing changed.
pub(super) fn message(stage: PreflightStage, cause: &PreflightCause) -> Message {
  if *cause == PreflightCause::Held {
    return Message::new("localIntegration.preflight.integrationLockHeld");
  }
  let key = match stage {
    PreflightStage::IntegrationLock => "localIntegration.preflight.integrationLock",
    PreflightStage::ProjectDatabase => "localIntegration.preflight.projectDatabase",
    PreflightStage::ScratchFolder => "localIntegration.preflight.scratchFolder",
    PreflightStage::CheckoutIdentity => "localIntegration.preflight.checkoutIdentity",
    PreflightStage::GitMetadata => "localIntegration.preflight.gitMetadata",
    PreflightStage::ExistingResult => "localIntegration.preflight.existingResult",
    PreflightStage::Policy => "localIntegration.preflight.policy",
  };
  Message::new(key).with("cause", cause.to_string())
}

/// Names `stage` alone, for an MCP client that is told no cause.
pub fn stage_message(stage: PreflightStage) -> Message {
  Message::new("localIntegration.preflight.stage").with("stage", stage.name().to_owned())
}
